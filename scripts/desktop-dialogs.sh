#!/usr/bin/env bash
# Самопроверка с настоящими системными диалогами файлов.
#
# Приложение запускается в сценарии `dialogs`: само вызывает «Сохранить» и
# «Открыть», а этот скрипт проходит открывшиеся системные диалоги так, как это
# сделал бы человек с клавиатуры:
#   macOS   — AppleScript (System Events): NSSavePanel / NSOpenPanel;
#   Windows — WScript.Shell SendKeys: стандартные диалоги Windows;
#   Linux   — xdotool: GtkFileChooserDialog (нужен X-сервер с оконным менеджером).
# Итог проверяется по файлу на диске и отчёту приложения.
#
# Использование: scripts/desktop-dialogs.sh <путь к бинарнику>
set -euo pipefail

app="$1"
work="$(mktemp -d)"
report="$work/report.txt"
target_dir="$work/projects"
mkdir -p "$target_dir"
target="$target_dir/selftest.oplan"

case "$(uname -s)" in
  Darwin) os=mac ;;
  MINGW* | MSYS* | CYGWIN*) os=windows ;;
  *) os=linux ;;
esac

native_path() {
  if [[ $os == windows ]]; then cygpath -w "$1"; else echo "$1"; fi
}

# Нажатия клавиш в активном окне ─────────────────────────────────────────────

mac_keys() {
  osascript -e 'tell application "System Events"' "$@" -e 'end tell'
}

win_send() {
  # Экранирование спецсимволов SendKeys: + ^ % ~ ( ) { } [ ]
  local text
  text=$(printf '%s' "$1" | sed -e 's/[][+^%~(){}]/{&}/g')
  powershell -NoProfile -Command "
    \$ws = New-Object -ComObject WScript.Shell
    [void]\$ws.AppActivate('OpenPlanner')
    Start-Sleep -Milliseconds 500
    # Alt+N — фокус в поле «Имя файла» (в диалоге открытия он стоит в списке).
    \$ws.SendKeys('$2')
    Start-Sleep -Milliseconds 300
    \$ws.SendKeys('$text')
    Start-Sleep -Milliseconds 300
    \$ws.SendKeys('{ENTER}')"
}

pass_save_dialog() {
  local dir name
  dir="$(native_path "$target_dir")"
  name="selftest"
  case $os in
    mac)
      mac_keys \
        -e "set frontmost of (first process whose unix id is $pid) to true" \
        -e 'delay 0.5' \
        -e 'keystroke "g" using {command down, shift down}' \
        -e 'delay 1' \
        -e "keystroke \"$dir\"" \
        -e 'delay 0.5' \
        -e 'key code 36' \
        -e 'delay 1' \
        -e 'keystroke "a" using command down' \
        -e "keystroke \"$name\"" \
        -e 'delay 0.5' \
        -e 'key code 36'
      ;;
    windows) win_send "$dir\\$name" '%n^a' ;;
    linux)
      xdotool key --clearmodifiers ctrl+a
      xdotool type --delay 20 "$dir/$name"
      sleep 0.5
      xdotool key Return
      ;;
  esac
}

pass_open_dialog() {
  local file
  file="$(native_path "$target")"
  case $os in
    mac)
      mac_keys \
        -e "set frontmost of (first process whose unix id is $pid) to true" \
        -e 'delay 0.5' \
        -e 'keystroke "g" using {command down, shift down}' \
        -e 'delay 1' \
        -e "keystroke \"$file\"" \
        -e 'delay 0.5' \
        -e 'key code 36' \
        -e 'delay 1.5' \
        -e 'key code 36'
      ;;
    windows) win_send "$file" '%n^a' ;;
    linux)
      xdotool key --clearmodifiers ctrl+l
      xdotool type --delay 20 "$file"
      sleep 0.5
      xdotool key Return
      ;;
  esac
}

# Сценарий ────────────────────────────────────────────────────────────────────

export OPENPLANNER_SELFTEST="$(native_path "$report")"
export OPENPLANNER_SELFTEST_SCENARIO=dialogs
: >"$report"

"$app" &
pid=$!
trap 'kill $pid 2>/dev/null || true' EXIT

wait_line() {
  for _ in $(seq 1 "${2:-90}"); do
    if grep -q "$1" "$report"; then return 0; fi
    if grep -qE '^(OK|FAIL) ' "$report"; then return 1; fi
    if ! kill -0 "$pid" 2>/dev/null; then return 1; fi
    sleep 1
  done
  return 1
}

screenshot() {
  [[ -n "${SHOTS:-}" ]] || return 0
  mkdir -p "$SHOTS"
  case $os in
    windows)
      powershell -NoProfile -Command "
        Add-Type -AssemblyName System.Windows.Forms, System.Drawing
        \$b = [System.Windows.Forms.SystemInformation]::VirtualScreen
        \$img = New-Object System.Drawing.Bitmap \$b.Width, \$b.Height
        [System.Drawing.Graphics]::FromImage(\$img).CopyFromScreen(\$b.Left, \$b.Top, 0, 0, \$img.Size)
        \$img.Save('$(native_path "$SHOTS")\\$1.png')" || true ;;
    mac) screencapture -x "$SHOTS/$1.png" || true ;;
    linux) import -window root "$SHOTS/$1.png" || true ;;
  esac
}

# Проходит диалог: ввод повторяется, пока приложение не подтвердит шаг
# (первый ввод мог уйти раньше, чем диалог получил фокус).
pass_dialog() {
  local action="$1" expect="$2"
  for attempt in 1 2 3 4; do
    sleep $((attempt + 1))
    screenshot "$action-$attempt"
    "$action"
    if wait_line "$expect" 8; then return 0; fi
  done
  return 1
}

fail() {
  screenshot failure
  echo "диалоги: $1" >&2
  echo "--- отчёт приложения:" >&2
  cat "$report" >&2
  exit 1
}

wait_line 'STEP dialog:save' || fail "приложение не дошло до сохранения"
pass_dialog pass_save_dialog 'STEP saved:' || fail "файл не сохранён через диалог"
[[ -s "$target" ]] || fail "файла нет на диске: $target"
echo "✓ системный диалог сохранения: $(grep 'STEP saved:' "$report" | sed 's/STEP saved://')"

wait_line 'STEP dialog:open' 30 || fail "приложение не дошло до открытия"
pass_dialog pass_open_dialog 'STEP opened' || fail "файл не открыт через диалог"
wait_line '^OK ' 30 || fail "приложение не завершило сценарий"
echo "✓ системный диалог открытия: проект открыт, название из файла совпало"
cat "$report"
