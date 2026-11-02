#!/usr/bin/env bash
# E2E настоящего десктопного приложения на Linux: Xvfb + openbox + tauri-driver.
# Требует: xvfb, openbox, xdotool, imagemagick, webkit2gtk-driver, tauri-driver
# (cargo install tauri-driver --locked) и собранное приложение
# (cd apps/desktop && bunx tauri build --debug --no-bundle).
set -euo pipefail

cd "$(dirname "$0")/.."
root=$(pwd)
app="${APP:-$root/target/debug/openplanner-desktop}"
tmp=$(mktemp -d)
export APP="$app" WORK="$tmp/work" SHOTS="${SHOTS:-$root/apps/desktop/test-results/desktop}"
# Отдельный HOME: чистая папка данных приложения (журнал восстановления).
export HOME="$tmp/home" XDG_DATA_HOME="$tmp/home/.local/share" XDG_CONFIG_HOME="$tmp/home/.config"
export LANG=C.UTF-8 LC_ALL=C.UTF-8 DISPLAY=:99
mkdir -p "$HOME"

Xvfb :99 -screen 0 1600x1000x24 >/dev/null 2>&1 &
xvfb=$!
cleanup() {
  kill "${driver:-}" "${wm:-}" "$xvfb" 2>/dev/null || true
  pkill -9 -f openplanner-desktop 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup EXIT
sleep 1
openbox >/dev/null 2>&1 &
wm=$!
tauri-driver --native-driver "$(command -v WebKitWebDriver)" >/dev/null 2>&1 &
driver=$!
sleep 2

node apps/desktop/e2e-desktop/run.mjs
