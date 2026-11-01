#!/usr/bin/env bash
# Проверка Conventional Commits: тип(скоуп)!: описание
set -euo pipefail

first_line=$(head -n1 "$1")
pattern='^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-z0-9-]+\))?!?: .+'

if [[ "$first_line" =~ ^(Merge|Revert|fixup!|squash!) ]] || [[ "$first_line" =~ $pattern ]]; then
  exit 0
fi

echo "Сообщение коммита не соответствует Conventional Commits:" >&2
echo "  $first_line" >&2
echo "Пример: feat(scan): распознавание дверных проёмов" >&2
exit 1
