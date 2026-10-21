#!/bin/bash
# run.sh [N] [workdir] — N прогонов задачи из fixture/ в вариантах edit и medit.
# Пользовательские настройки (~/.claude/CLAUDE.md, плагины) отключены через --setting-sources project,
# medit из PATH убран — в варианте medit он берётся только из плагина.
B=$(cd "$(dirname "$0")" && pwd)
N=${1:-3}
W=${2:-/tmp/medit-bench}
CL=$(command -v claude)
CL=$(readlink -f "$CL")
export PATH="/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin"
TASK='В проекте в src/ сделай рефакторинг:
1) переименуй функцию fetchUser в loadUser везде — определение, импорты и все вызовы;
2) в src/config.ts поменяй timeout на 10000 и retries на 5;
3) во всех файлах src, где есть console.log, замени каждый console.log на logger.info и добавь первой строкой файла импорт: import { logger } from "./logger";
4) создай src/logger.ts с содержимым: export const logger = { info: console.log };
Больше ничего не меняй. В конце ответь одним словом: готово.'
HINT='Когда нужно поправить несколько мест или файлов за один вызов — используй medit (см. skill medit), а не Python-скрипт.'
total=$((N * 2)); i=0
mkdir -p "$W"
for r in $(seq 1 "$N"); do
  for arm in edit medit; do
    i=$((i + 1)); echo "[$i/$total] $arm run $r"
    w=$W/$arm-$r; rm -rf "$w"; cp -r "$B/fixture" "$w"; cd "$w"
    if [ $arm = edit ]; then
      echo "$TASK" | "$CL" -p --setting-sources project --output-format stream-json --verbose \
        --allowedTools "Read" "Edit" "Write" "Grep" "Glob" --disallowedTools "Bash" > "$W/$arm-$r.jsonl" 2>&1
    else
      echo "$TASK" | "$CL" -p --setting-sources project --output-format stream-json --verbose \
        --plugin-dir "$B/.." --append-system-prompt "$HINT" \
        --allowedTools "Read" "Edit" "Write" "Grep" "Glob" "Bash(medit:*)" "Skill" > "$W/$arm-$r.jsonl" 2>&1
    fi
    "$B/check.sh" "$w" > "$W/$arm-$r.check"
  done
done
"$B/summary.sh" "$W"
