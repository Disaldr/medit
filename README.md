# medit

Пакетная правка файлов одним вызовом — CLI для Claude Code (и для людей).

Вместо серии `Edit` или Python-скрипта «прочитать → заменить → записать» Claude отправляет одну heredoc-команду с любым числом замен в любом числе файлов. Применяется всё или ничего: если хоть одна замена не нашлась, ни один файл не меняется.

```
medit <<'EOF'
=== src/app.ts
<<<
const port = 3000;
>>>
const port = Number(process.env.PORT ?? 3000);
<<<*
console.log(
>>>
logger.info(
+++ src/logger.ts
export const logger = console;
EOF
```

```
src/app.ts: 2 ok (L4 L17)
src/logger.ts: создан, строк: 1
```

## Установка как плагин Claude Code

В терминальной сессии Claude Code:

```
/plugin install medit --marketplace Disaldr/medit
```

На вопрос `Add marketplace?` ответить `y`, затем выбрать scope (user — для всех проектов).

Или из обычного шелла:

```
claude plugin install medit --marketplace Disaldr/medit
```

Плагин содержит:

- `bin/medit` — готовый универсальный бинарник для macOS (Apple Silicon + Intel). Claude Code сам добавляет его в PATH для Bash, ничего собирать не нужно.
- skill `medit` — инструкция для Claude: когда и как пользоваться утилитой.

Чтобы Claude брался за `medit` активнее, можно добавить в `~/.claude/CLAUDE.md` строку:

```
Когда нужно поправить несколько мест или файлов за один вызов — используй medit (см. skill medit), а не Python-скрипт.
```

## Формат

```
medit [-n|--dry-run] [-C dir] [script]      # script по умолчанию — stdin
```

| Маркер | Значение |
| --- | --- |
| `=== path` | дальше идут правки этого файла |
| `<<<` | старый текст, должен встречаться ровно 1 раз |
| `<<<*` | старый текст, заменить все вхождения (минимум 1) |
| `>>>` | новый текст (пусто — удалить) |
| `+++ path` | создать файл с содержимым до следующего маркера (ошибка, если уже есть) |
| `+++! path` | создать или перезаписать |

- Маркеры — целые строки. Блок кончается перед следующим маркером, его последний перевод строки в текст не входит. Пустые строки между блоками войдут в текст — не ставьте их.
- Замены в одном файле применяются по порядку; можно создать файл и тут же его править.
- При ошибке: `exit 1` и сообщение `script:N: file: причина`; если старый текст не найден, но есть совпадение без учёта отступов — будет подсказка.
- `--dry-run` проверяет всё, но ничего не пишет.
- Запись атомарная (временный файл + rename), права файлов сохраняются, новые файлы создаются с учётом umask.
- Экранирования нет: текст, содержащий строки-маркеры, через medit не записать.

## Сборка из исходников

Нужен Rust (edition 2024). Без зависимостей.

```
cargo install --git https://github.com/Disaldr/medit
```

Пересобрать универсальный бинарник для плагина:

```
rustup target add x86_64-apple-darwin aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
lipo -create -output bin/medit \
  target/x86_64-apple-darwin/release/medit \
  target/aarch64-apple-darwin/release/medit
```

На Linux готового бинарника нет: соберите `cargo build --release` и положите результат в `bin/medit` или в любой каталог из PATH.

## Лицензия

MIT
