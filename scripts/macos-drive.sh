# Shared launcher for the macOS P0 jobs. Source this file.
# Real CGEvents only. A focus or key failure prints the tool error and returns 1.

drive_root() {
  git rev-parse --show-toplevel
}

drive_init() {
  DRIVE_ROOT="$(drive_root)"
  DRIVE_APP="${1:?app path}"
  DRIVE_WORK="$(mktemp -d)"
  DRIVE_PROBE="$DRIVE_WORK/probe.txt"
  DRIVE_LOG="$DRIVE_WORK/lux.log"
  DRIVE_PID=""
  cat > /tmp/lux-find-window.swift << 'SWIFT'
import CoreGraphics
import Foundation
let want = CommandLine.arguments.dropFirst().first ?? ""
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let info = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else { exit(1) }
for w in info {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    if layer != 0 || owner.caseInsensitiveCompare(want) != .orderedSame { continue }
    guard let bounds = w[kCGWindowBounds as String] as? [String: Any] else { continue }
    let width = (bounds["Width"] as? NSNumber)?.doubleValue ?? 0
    let height = (bounds["Height"] as? NSNumber)?.doubleValue ?? 0
    if width < 200 || height < 200 { continue }
    let x = (bounds["X"] as? NSNumber)?.doubleValue ?? 0
    let y = (bounds["Y"] as? NSNumber)?.doubleValue ?? 0
    print("\(Int(x)) \(Int(y)) \(Int(width)) \(Int(height))")
    exit(0)
}
exit(1)
SWIFT
  cat > /tmp/lux-drive.swift << 'SWIFT'
import CoreGraphics
import Foundation
func flags(_ raw: String) -> CGEventFlags {
    var out = CGEventFlags()
    for part in raw.split(separator: ",") {
        switch part {
        case "cmd": out.insert(.maskCommand)
        case "shift": out.insert(.maskShift)
        case "alt": out.insert(.maskAlternate)
        case "ctrl": out.insert(.maskControl)
        default: break
        }
    }
    return out
}
func post(_ key: CGKeyCode, _ down: Bool, _ fl: CGEventFlags) {
    let source = CGEventSource(stateID: .hidSystemState)
    guard let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: down) else {
        fputs("cannot build a key event\n", stderr)
        exit(1)
    }
    event.flags = fl
    event.post(tap: .cghidEventTap)
}
func holdModifiers(_ fl: CGEventFlags, _ down: Bool) {
    // Flags on the letter event are not enough. A Command-P that never
    // releases Command makes the next Command-Shift-P look like Command-P.
    if down {
        if fl.contains(.maskCommand) { post(55, true, []) }
        if fl.contains(.maskShift) { post(56, true, []) }
        if fl.contains(.maskAlternate) { post(58, true, []) }
        if fl.contains(.maskControl) { post(59, true, []) }
    } else {
        if fl.contains(.maskControl) { post(59, false, []) }
        if fl.contains(.maskAlternate) { post(58, false, []) }
        if fl.contains(.maskShift) { post(56, false, []) }
        if fl.contains(.maskCommand) { post(55, false, []) }
    }
}
func type(_ text: String) {
    let units = Array(text.utf16)
    let source = CGEventSource(stateID: .hidSystemState)
    guard let down = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: true),
          let up = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: false) else {
        fputs("cannot build a text event\n", stderr)
        exit(1)
    }
    down.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
    up.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
    down.flags = []
    up.flags = []
    down.post(tap: .cghidEventTap)
    up.post(tap: .cghidEventTap)
}
let args = CommandLine.arguments
guard args.count >= 2 else { fputs("missing step\n", stderr); exit(2) }
switch args[1] {
case "type":
    guard args.count >= 3 else { exit(2) }
    type(args[2])
case "chars":
    guard args.count >= 3 else { exit(2) }
    for ch in args[2] {
        type(String(ch))
        Thread.sleep(forTimeInterval: 0.03)
    }
case "tap":
    guard args.count >= 4, let code = UInt16(args[2]) else { exit(2) }
    let fl = flags(args[3])
    holdModifiers(fl, true)
    if fl.rawValue != 0 { Thread.sleep(forTimeInterval: 0.05) }
    post(CGKeyCode(code), true, fl)
    Thread.sleep(forTimeInterval: 0.03)
    post(CGKeyCode(code), false, fl)
    holdModifiers(fl, false)
case "chord":
    // Hold the modifiers, then tap each key. A two-step chord such as
    // Command-K Command-D is one held Command, not two separate taps.
    guard args.count >= 4 else { exit(2) }
    let fl = flags(args[2])
    holdModifiers(fl, true)
    Thread.sleep(forTimeInterval: 0.05)
    for raw in args.dropFirst(3) {
        guard let code = UInt16(raw) else { exit(2) }
        post(CGKeyCode(code), true, fl)
        Thread.sleep(forTimeInterval: 0.03)
        post(CGKeyCode(code), false, fl)
        Thread.sleep(forTimeInterval: 0.08)
    }
    holdModifiers(fl, false)
case "scroll":
    let x = Double(args.count > 2 ? args[2] : "400") ?? 400
    let y = Double(args.count > 3 ? args[3] : "400") ?? 400
    let lines = Int32(args.count > 4 ? args[4] : "-8") ?? -8
    let point = CGPoint(x: x, y: y)
    let source = CGEventSource(stateID: .hidSystemState)
    if let move = CGEvent(mouseEventSource: source, mouseType: .mouseMoved, mouseCursorPosition: point, mouseButton: .left) {
        move.post(tap: .cghidEventTap)
    }
    Thread.sleep(forTimeInterval: 0.05)
    guard let wheel = CGEvent(scrollWheelEvent2Source: source, units: .line, wheelCount: 1, wheel1: lines, wheel2: 0, wheel3: 0) else {
        fputs("cannot build a scroll event\n", stderr)
        exit(1)
    }
    wheel.location = point
    wheel.post(tap: .cghidEventTap)
case "scrollx", "scrollshift":
    let x = Double(args.count > 2 ? args[2] : "400") ?? 400
    let y = Double(args.count > 3 ? args[3] : "400") ?? 400
    let lines = Int32(args.count > 4 ? args[4] : "-8") ?? -8
    let point = CGPoint(x: x, y: y)
    let source = CGEventSource(stateID: .hidSystemState)
    if let move = CGEvent(mouseEventSource: source, mouseType: .mouseMoved, mouseCursorPosition: point, mouseButton: .left) {
        move.post(tap: .cghidEventTap)
    }
    Thread.sleep(forTimeInterval: 0.05)
    let vertical: Int32 = args[1] == "scrollx" ? 0 : lines
    let horizontal: Int32 = args[1] == "scrollx" ? lines : 0
    guard let wheel = CGEvent(scrollWheelEvent2Source: source, units: .line, wheelCount: 2, wheel1: vertical, wheel2: horizontal, wheel3: 0) else {
        fputs("cannot build a scroll event\n", stderr)
        exit(1)
    }
    if args[1] == "scrollshift" { wheel.flags = .maskShift }
    wheel.location = point
    wheel.post(tap: .cghidEventTap)
default:
    fputs("unknown step\n", stderr)
    exit(2)
}
SWIFT
}

drive_stop() {
  if [[ -n "${DRIVE_PID:-}" ]] && kill -0 "$DRIVE_PID" 2>/dev/null; then
    kill "$DRIVE_PID" 2>/dev/null || true
    wait "$DRIVE_PID" 2>/dev/null || true
  fi
  DRIVE_PID=""
}

drive_focus() {
  local err="$DRIVE_WORK/focus.err"
  if ! osascript -e "tell application \"System Events\" to set frontmost of process \"$DRIVE_OWNER\" to true" >"$err" 2>&1; then
    echo "System Events refused to focus Lux.app (process $DRIVE_OWNER):" >&2
    cat "$err" >&2
    return 1
  fi
}

drive_wait_window() {
  DRIVE_OWNER=""
  local _
  for _ in $(seq 1 60); do
    if ! kill -0 "$DRIVE_PID" 2>/dev/null; then
      echo "Lux.app exited before a window appeared" >&2
      cat "$DRIVE_LOG" >&2 || true
      return 1
    fi
    local candidate
    for candidate in Lux lux; do
      if swift /tmp/lux-find-window.swift "$candidate" >/dev/null 2>&1; then
        DRIVE_OWNER="$candidate"
        break
      fi
    done
    if [[ -n "$DRIVE_OWNER" ]]; then
      return 0
    fi
    sleep 1
  done
  echo "timed out waiting for Lux.app" >&2
  cat "$DRIVE_LOG" >&2 || true
  return 1
}

# drive_launch FILE [extra lux args...]
drive_launch() {
  local file="$1"
  shift
  drive_stop
  rm -f "$DRIVE_PROBE"
  : > "$DRIVE_LOG"
  "$DRIVE_APP/Contents/MacOS/lux" --probe "$DRIVE_PROBE" "$@" "$file" >"$DRIVE_LOG" 2>&1 &
  DRIVE_PID=$!
  drive_wait_window
  sleep 0.3
  drive_focus
}

drive_key() {
  drive_focus
  if ! swift /tmp/lux-drive.swift "$@"; then
    echo "CGEvent failed: $*" >&2
    return 1
  fi
}

drive_dump() {
  rm -f "$DRIVE_PROBE"
  kill -USR2 "$DRIVE_PID"
  local attempt sent=1
  for attempt in $(seq 1 60); do
    if [[ -s "$DRIVE_PROBE" ]]; then
      return 0
    fi
    if ! kill -0 "$DRIVE_PID" 2>/dev/null; then
      echo "Lux.app exited while waiting for the probe" >&2
      cat "$DRIVE_LOG" >&2 || true
      return 1
    fi
    sleep 0.25
    # `$_` is the previous command's last argument, not this loop variable.
    if [[ "$sent" -eq 1 && "$attempt" -eq 8 ]]; then
      kill -USR2 "$DRIVE_PID" || true
      sent=2
    fi
  done
  echo "timed out waiting for $DRIVE_PROBE" >&2
  tail -40 "$DRIVE_LOG" >&2 || true
  return 1
}

drive_field() {
  local key="$1"
  sed -n "s/^${key}=//p" "$DRIVE_PROBE" | head -1
}

drive_save() {
  drive_key tap 1 cmd
  sleep 0.6
}

# One trailing newline is not part of the assertion. The editor may round
# the fixture's final newline when it saves.
drive_norm() {
  local text="$1"
  printf '%s' "${text%$'\n'}"
}

drive_scroll() {
  local lines="${1:--8}"
  local line x y w h
  line="$(swift /tmp/lux-find-window.swift "$DRIVE_OWNER")"
  read -r x y w h <<<"$line"
  local cx=$((x + w / 2))
  local cy=$((y + h / 2))
  drive_key scroll "$cx" "$cy" "$lines"
}

drive_chord() {
  drive_key chord "$@"
}
