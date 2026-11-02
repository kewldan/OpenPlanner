#!/usr/bin/env bash
# Самопроверка с настоящими системными диалогами файлов и восстановлением после сбоя.
#
# Приложение запускается трижды, в сценариях самопроверки:
#   dialogs — само вызывает «Сохранить» и «Открыть»;
#   crash   — открывает файл, вносит правку, ждёт записи журнала; затем kill -9;
#   recover — открывает файл, системный вопрос о восстановлении, сверка правки.
# Этот скрипт проходит открывшиеся системные окна так, как это сделал бы
# человек с клавиатуры:
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

# Окно приложения на передний план. Windows не даёт новому процессу перехватить
# фокус (поверх висит консоль агента раннера): сворачиваем чужие окна и вызываем
# SetForegroundWindow после нажатия Alt — стандартный способ снять блокировку.
WIN_FOCUS='
  Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class W {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, UIntPtr e);
}
"@
  Get-Process | Where-Object {
    $_.MainWindowHandle -ne 0 -and $_.ProcessName -notin @("openplanner-desktop", "explorer")
  } | ForEach-Object { [void][W]::ShowWindow($_.MainWindowHandle, 6) }
  $app = Get-Process openplanner-desktop -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
  [W]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero)
  [W]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
  if ($app) { [void][W]::SetForegroundWindow($app.MainWindowHandle) }
  Start-Sleep -Milliseconds 500
  $ws = New-Object -ComObject WScript.Shell
'

win_send() {
  # Путь вставляется из буфера обмена: при посимвольном вводе автодополнение
  # поля «Имя файла» съедает часть символов.
  OP_TEXT="$1" powershell -NoProfile -Command "$WIN_FOCUS"'
    Set-Clipboard -Value $env:OP_TEXT
    # Alt+N — фокус в поле «Имя файла» (в диалоге открытия он стоит в списке).
    $ws.SendKeys("%n")
    Start-Sleep -Milliseconds 300
    $ws.SendKeys("^a^v")
    Start-Sleep -Milliseconds 500
    $ws.SendKeys("{ENTER}")'
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
    windows) win_send "$dir\\$name.oplan" ;;
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
    windows) win_send "$file" ;;
    linux)
      xdotool key --clearmodifiers ctrl+l
      xdotool type --delay 20 "$file"
      sleep 0.5
      xdotool key Return
      ;;
  esac
}

press_enter() {
  case $os in
    mac)
      mac_keys \
        -e "set frontmost of (first process whose unix id is $pid) to true" \
        -e 'delay 0.5' \
        -e 'key code 36'
      ;;
    windows)
      powershell -NoProfile -Command "$WIN_FOCUS"'
        $ws.SendKeys("{ENTER}")'
      ;;
    linux) xdotool key Return ;;
  esac
}

# Открыть файл и согласиться на восстановление: Enter нажимает кнопку по
# умолчанию («Восстановить») в системном вопросе.
pass_open_and_recover() {
  pass_open_dialog
  sleep 3
  screenshot recovery-question
  press_enter
}

# Запуски приложения ─────────────────────────────────────────────────────────

pid=""
trap 'kill $pid 2>/dev/null || true' EXIT

start_app() {
  export OPENPLANNER_SELFTEST="$(native_path "$report")"
  export OPENPLANNER_SELFTEST_SCENARIO="$1"
  : >"$report"
  "$app" &
  pid=$!
}

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

# Проходит системное окно: ввод повторяется, пока приложение не подтвердит шаг
# (первый ввод мог уйти раньше, чем окно получило фокус).
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

# 1. Сохранение и открытие ────────────────────────────────────────────────────
start_app dialogs
wait_line 'STEP dialog:save' || fail "приложение не дошло до сохранения"
pass_dialog pass_save_dialog 'STEP saved:' || fail "файл не сохранён через диалог"
[[ -s "$target" ]] || fail "файла нет на диске: $target"
echo "✓ системный диалог сохранения: $(grep 'STEP saved:' "$report" | sed 's/STEP saved://')"
wait_line 'STEP dialog:open' 30 || fail "приложение не дошло до открытия"
pass_dialog pass_open_dialog 'STEP opened' || fail "файл не открыт через диалог"
wait_line '^OK ' 30 || fail "приложение не завершило сценарий"
echo "✓ системный диалог открытия: проект открыт, название из файла совпало"
cat "$report"
wait "$pid" 2>/dev/null || true

# 2. Правка и аварийное завершение ────────────────────────────────────────────
journal="$target.journal"
start_app crash
wait_line 'STEP dialog:open' || fail "сбой: приложение не дошло до открытия"
pass_dialog pass_open_dialog 'STEP journaled' || fail "сбой: правка не записана в журнал"
grep -q 'Recovered' "$journal" || fail "сбой: в журнале нет правки ($journal)"
echo "✓ правка записана в журнал рядом с файлом"
kill -9 "$pid"
wait "$pid" 2>/dev/null || true
echo "✓ процесс аварийно завершён (kill -9)"

# 3. Восстановление ───────────────────────────────────────────────────────────
start_app recover
wait_line 'STEP dialog:open' || fail "восстановление: приложение не дошло до открытия"
pass_dialog pass_open_and_recover 'STEP recovered' || fail "восстановление: правка не восстановлена"
wait_line '^OK ' 30 || fail "восстановление: приложение не завершило сценарий"
echo "✓ системный вопрос о восстановлении: правка из журнала восстановлена, проект помечен несохранённым"
cat "$report"
