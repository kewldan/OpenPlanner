#!/usr/bin/env bash
# Самопроверка собранного десктопного приложения: запуск, старт движка, бэкенд GPU.
# Использование: [EXPECT_BACKEND=BrowserWebGpu] scripts/desktop-selftest.sh <путь к бинарнику>
set -euo pipefail

app="$1"
report="$(mktemp -d)/selftest.txt"
export OPENPLANNER_SELFTEST="$report"

"$app" &
pid=$!
for _ in $(seq 1 120); do
  [[ -s "$report" ]] && break
  if ! kill -0 "$pid" 2>/dev/null; then break; fi
  sleep 1
done
kill "$pid" 2>/dev/null || true

if [[ ! -s "$report" ]]; then
  echo "самопроверка: приложение не прислало отчёт" >&2
  exit 1
fi
cat "$report"
grep -q '^OK ' "$report"
# EXPECT_BACKEND — какой бэкенд обязан выбрать движок (например, BrowserWebGpu).
if [[ -n "${EXPECT_BACKEND:-}" ]] && ! grep -q "backend=$EXPECT_BACKEND " "$report"; then
  echo "самопроверка: ожидался бэкенд $EXPECT_BACKEND" >&2
  exit 1
fi
