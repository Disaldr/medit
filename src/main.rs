//! medit — пакетная правка файлов одним вызовом.
//!
//! Формат (stdin или файл-аргумент):
//!
//!   === path/to/file        правка существующего файла
//!   <<<                     старый текст (должен встречаться ровно 1 раз)
//!   old
//!   >>>
//!   new
//!   <<<*                    то же, но заменить все вхождения (минимум 1)
//!   old
//!   >>>
//!   new
//!   +++ path/to/new         создать файл (ошибка, если уже есть); +++! — перезаписать
//!   content
//!
//! Всё или ничего: при любой ошибке ни один файл не меняется.

use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const HELP: &str = "usage: medit [--dry-run] [-C dir] [script]   (script по умолчанию из stdin)

  === path      правка файла; далее блоки:
  <<<           старый текст, ровно 1 вхождение   (<<<* — все вхождения, >=1)
  >>>           новый текст (пустой = удалить)
  +++ path      создать файл с содержимым до следующего заголовка (+++! — перезаписать)

Маркеры — целые строки. Блок заканчивается перед следующим маркером,
последний перевод строки блока не входит в текст. Всё или ничего.";

struct Replace {
    old: String,
    new: String,
    all: bool,
    line: usize, // строка скрипта, для сообщений
}

enum Kind {
    Edit(Vec<Replace>),
    Create { content: String, overwrite: bool },
}

struct Section {
    path: String,
    kind: Kind,
    line: usize,
}

fn strip_eol(l: &str) -> &str {
    l.strip_suffix('\n').map(|s| s.strip_suffix('\r').unwrap_or(s)).unwrap_or(l)
}

/// Заголовок секции: ("===" | "+++" | "+++!", path).
fn header(l: &str) -> Option<(&str, &str)> {
    for tag in ["+++!", "===", "+++"] {
        if let Some(rest) = l.strip_prefix(tag) {
            if let Some(p) = rest.strip_prefix(' ') {
                let p = p.trim();
                if !p.is_empty() {
                    return Some((tag, p));
                }
            }
        }
    }
    None
}

fn block_text(lines: &[&str]) -> String {
    let mut s: String = lines.concat();
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
    s
}

fn parse(src: &str) -> Result<Vec<Section>, String> {
    let lines: Vec<&str> = src.split_inclusive('\n').collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let l = strip_eol(lines[i]);
        if l.trim().is_empty() {
            i += 1;
            continue;
        }
        let Some((tag, path)) = header(l) else {
            return Err(format!("script:{}: ожидался '=== path' или '+++ path', найдено: {l}", i + 1));
        };
        let line = i + 1;
        i += 1;
        if tag != "===" {
            let start = i;
            while i < lines.len() && header(strip_eol(lines[i])).is_none() {
                i += 1;
            }
            out.push(Section {
                path: path.to_string(),
                kind: Kind::Create { content: lines[start..i].concat(), overwrite: tag == "+++!" },
                line,
            });
            continue;
        }
        let mut ops = Vec::new();
        loop {
            while i < lines.len() && strip_eol(lines[i]).trim().is_empty() {
                i += 1;
            }
            if i >= lines.len() || header(strip_eol(lines[i])).is_some() {
                break;
            }
            let m = strip_eol(lines[i]);
            let all = match m {
                "<<<" => false,
                "<<<*" => true,
                _ => return Err(format!("script:{}: ожидался '<<<' в секции {path}, найдено: {m}", i + 1)),
            };
            let op_line = i + 1;
            i += 1;
            let start = i;
            while i < lines.len() && strip_eol(lines[i]) != ">>>" {
                i += 1;
            }
            if i >= lines.len() {
                return Err(format!("script:{op_line}: нет '>>>' после '<<<'"));
            }
            let old = block_text(&lines[start..i]);
            i += 1;
            let start = i;
            while i < lines.len() {
                let m = strip_eol(lines[i]);
                if m == "<<<" || m == "<<<*" || header(m).is_some() {
                    break;
                }
                i += 1;
            }
            let new = block_text(&lines[start..i]);
            ops.push(Replace { old, new, all, line: op_line });
        }
        if ops.is_empty() {
            return Err(format!("script:{line}: секция {path} без замен"));
        }
        out.push(Section { path: path.to_string(), kind: Kind::Edit(ops), line });
    }
    if out.is_empty() {
        return Err("пустой скрипт".into());
    }
    Ok(out)
}

fn line_of(s: &str, byte: usize) -> usize {
    s[..byte].bytes().filter(|&b| b == b'\n').count() + 1
}

/// Подсказка, если старый текст не найден: совпадение без учёта отступов или хотя бы первой строки.
fn hint(cur: &str, old: &str) -> String {
    let want: Vec<&str> = old.lines().map(str::trim).collect();
    let have: Vec<&str> = cur.lines().map(str::trim).collect();
    if !want.is_empty() && want.len() <= have.len() {
        let hits: Vec<String> = (0..=have.len() - want.len())
            .filter(|&k| have[k..k + want.len()] == want[..])
            .map(|k| (k + 1).to_string())
            .take(5)
            .collect();
        if !hits.is_empty() {
            return format!("; совпадает без учёта отступов/пробелов в строке {}", hits.join(","));
        }
    }
    if let Some(first) = want.iter().find(|w| !w.is_empty()) {
        let hits: Vec<String> = have
            .iter()
            .enumerate()
            .filter(|(_, h)| h == &first)
            .map(|(k, _)| (k + 1).to_string())
            .take(5)
            .collect();
        if !hits.is_empty() {
            return format!("; первая строка найдена в строке {}, расходится дальше", hits.join(","));
        }
    }
    String::new()
}

struct FileState {
    path: PathBuf,
    shown: String,
    cur: String,
    exists: bool,
    created: bool,
    hits: Vec<usize>,
    errors: usize,
}

fn run(script: &str, base: &Path, dry: bool) -> Result<String, String> {
    let sections = parse(script)?;
    let mut files: Vec<FileState> = Vec::new();
    let mut errs: Vec<String> = Vec::new();

    for sec in sections {
        let path = base.join(&sec.path);
        let idx = match files.iter().position(|f| f.path == path) {
            Some(k) => k,
            None => {
                let (cur, exists) = match fs::read_to_string(&path) {
                    Ok(s) => (s, true),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => (String::new(), false),
                    Err(e) => {
                        errs.push(format!("{}: {e}", sec.path));
                        continue;
                    }
                };
                files.push(FileState {
                    path,
                    shown: sec.path.clone(),
                    cur,
                    exists,
                    created: false,
                    hits: vec![],
                    errors: 0,
                });
                files.len() - 1
            }
        };
        let f = &mut files[idx];
        match sec.kind {
            Kind::Create { content, overwrite } => {
                if (f.exists || f.created) && !overwrite {
                    errs.push(format!("script:{}: {} уже существует (используйте +++!)", sec.line, f.shown));
                    f.errors += 1;
                    continue;
                }
                f.cur = content;
                f.created = true;
            }
            Kind::Edit(ops) => {
                if !f.exists && !f.created {
                    errs.push(format!("script:{}: {}: файл не найден", sec.line, f.shown));
                    f.errors += 1;
                    continue;
                }
                for op in ops {
                    if op.old.is_empty() {
                        errs.push(format!("script:{}: {}: пустой старый текст", op.line, f.shown));
                        f.errors += 1;
                        continue;
                    }
                    let pos: Vec<usize> = f.cur.match_indices(&op.old).map(|(p, _)| p).collect();
                    if pos.is_empty() {
                        errs.push(format!("script:{}: {}: не найдено{}", op.line, f.shown, hint(&f.cur, &op.old)));
                        f.errors += 1;
                        continue;
                    }
                    if !op.all && pos.len() > 1 {
                        let ls: Vec<String> = pos.iter().take(8).map(|&p| line_of(&f.cur, p).to_string()).collect();
                        errs.push(format!(
                            "script:{}: {}: {} вхождений (строки {}); расширьте контекст или используйте <<<*",
                            op.line, f.shown, pos.len(), ls.join(",")
                        ));
                        f.errors += 1;
                        continue;
                    }
                    f.cur = f.cur.replace(&op.old, &op.new);
                    // строка начала каждой замены в файле сразу после этой замены
                    let delta = op.new.len() as isize - op.old.len() as isize;
                    for (k, &p) in pos.iter().enumerate() {
                        f.hits.push(line_of(&f.cur, (p as isize + delta * k as isize) as usize));
                    }
                }
            }
        }
    }

    if !errs.is_empty() {
        return Err(format!("{}\nничего не записано", errs.join("\n")));
    }

    let mut report = Vec::new();
    for f in &files {
        let what = if f.created {
            format!("{}: {}, строк: {}", f.shown, if f.exists { "перезаписан" } else { "создан" }, f.cur.lines().count())
        } else {
            let ls: Vec<String> = f.hits.iter().map(|l| format!("L{l}")).collect();
            format!("{}: {} ok ({})", f.shown, f.hits.len(), ls.join(" "))
        };
        report.push(what);
        if dry {
            continue;
        }
        if let Err(e) = write_atomic(&f.path, &f.cur) {
            return Err(format!("{}: запись не удалась: {e}\nчасть файлов могла быть записана: {}", f.shown, report.join("; ")));
        }
    }
    if dry {
        report.push("dry-run: ничего не записано".into());
    }
    Ok(report.join("\n"))
}

fn write_atomic(path: &Path, content: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            fs::create_dir_all(dir)?;
        }
    }
    use std::os::unix::fs::OpenOptionsExt;
    let perms = fs::metadata(path).ok().map(|m| m.permissions());
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let tmp = path.with_file_name(format!(".{name}.medit.{}.{nonce}", std::process::id()));
    // create_new (O_EXCL) не идёт по симлинкам и не трогает чужой файл.
    // Существующий файл: 0o600 до записи, затем его исходные права; новый — 0o666 с учётом umask.
    let res = (|| {
        let mode = if perms.is_some() { 0o600 } else { 0o666 };
        let mut w = fs::OpenOptions::new().write(true).create_new(true).mode(mode).open(&tmp)?;
        if let Some(p) = perms {
            w.set_permissions(p)?;
        }
        w.write_all(content.as_bytes())?;
        w.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    res
}

fn main() -> ExitCode {
    let mut dry = false;
    let mut base = PathBuf::from(".");
    let mut script_path: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => {
                println!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "-n" | "--dry-run" => dry = true,
            "-C" => match args.next() {
                Some(d) => base = PathBuf::from(d),
                None => {
                    eprintln!("medit: -C требует каталог");
                    return ExitCode::from(2);
                }
            },
            _ if script_path.is_none() && !a.starts_with('-') => script_path = Some(a),
            _ => {
                eprintln!("medit: неизвестный аргумент {a}\n{HELP}");
                return ExitCode::from(2);
            }
        }
    }
    let script = match &script_path {
        Some(p) => fs::read_to_string(p).map_err(|e| format!("{p}: {e}")),
        None => {
            let mut s = String::new();
            io::stdin().read_to_string(&mut s).map(|_| s).map_err(|e| e.to_string())
        }
    };
    let result = script.and_then(|s| run(&s, &base, dry));
    match result {
        Ok(r) => {
            println!("{r}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("medit-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn replace_multi_file() {
        let d = tmpdir("multi");
        fs::write(d.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        fs::write(d.join("b.txt"), "x = 1\ny = 1\n").unwrap();
        let s = "=== a.txt\n<<<\ntwo\n>>>\nTWO\n2\n<<<\nthree\n>>>\n=== b.txt\n<<<*\n= 1\n>>>\n= 2\n";
        let r = run(s, &d, false).unwrap();
        assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "one\nTWO\n2\n\n");
        assert_eq!(fs::read_to_string(d.join("b.txt")).unwrap(), "x = 2\ny = 2\n");
        assert!(r.contains("a.txt: 2 ok (L2 L4)"), "{r}");
        assert!(r.contains("b.txt: 2 ok (L1 L2)"), "{r}");
    }

    #[test]
    fn all_or_nothing() {
        let d = tmpdir("atomic");
        fs::write(d.join("a.txt"), "foo\n").unwrap();
        fs::write(d.join("b.txt"), "dup\ndup\n").unwrap();
        let s = "=== a.txt\n<<<\nfoo\n>>>\nbar\n=== b.txt\n<<<\ndup\n>>>\nx\n";
        let e = run(s, &d, false).unwrap_err();
        assert!(e.contains("2 вхождений (строки 1,2)"), "{e}");
        assert!(e.contains("ничего не записано"));
        assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "foo\n");
    }

    #[test]
    fn not_found_hint() {
        let d = tmpdir("hint");
        fs::write(d.join("a.go"), "func f() {\n\treturn 1\n}\n").unwrap();
        let e = run("=== a.go\n<<<\nfunc f() {\n  return 1\n>>>\nx\n", &d, false).unwrap_err();
        assert!(e.contains("без учёта отступов/пробелов в строке 1"), "{e}");
        let e = run("=== a.go\n<<<\nfunc f() {\n\treturn 2\n>>>\nx\n", &d, false).unwrap_err();
        assert!(e.contains("первая строка найдена в строке 1"), "{e}");
    }

    #[test]
    fn create_and_edit_same_batch() {
        let d = tmpdir("create");
        let s = "+++ sub/new.txt\nhello\nworld\n=== sub/new.txt\n<<<\nworld\n>>>\nthere\n";
        let r = run(s, &d, false).unwrap();
        assert_eq!(fs::read_to_string(d.join("sub/new.txt")).unwrap(), "hello\nthere\n");
        assert!(r.contains("создан"), "{r}");
        let e = run("+++ sub/new.txt\nx\n", &d, false).unwrap_err();
        assert!(e.contains("уже существует"));
        run("+++! sub/new.txt\nx\n", &d, false).unwrap();
        assert_eq!(fs::read_to_string(d.join("sub/new.txt")).unwrap(), "x\n");
    }

    #[test]
    fn dry_run_and_missing() {
        let d = tmpdir("dry");
        fs::write(d.join("a.txt"), "a\n").unwrap();
        let r = run("=== a.txt\n<<<\na\n>>>\nb\n", &d, true).unwrap();
        assert!(r.contains("dry-run"));
        assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "a\n");
        let e = run("=== nope.txt\n<<<\na\n>>>\nb\n", &d, false).unwrap_err();
        assert!(e.contains("файл не найден"));
        assert!(parse("garbage\n").is_err());
        assert!(parse("=== a.txt\n<<<\nno end\n").is_err());
    }

    #[test]
    fn preserves_permissions() {
        let d = tmpdir("perm");
        let p = d.join("run.sh");
        fs::write(&p, "echo a\n").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        run("=== run.sh\n<<<\na\n>>>\nb\n", &d, false).unwrap();
        assert_eq!(fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o755);
    }
}
