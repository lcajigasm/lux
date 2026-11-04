#!/usr/bin/env bash
# Real keystrokes against the compiled Lux app on macOS.
# Does not open panels with --palette or --query. Selects the Spanish
# layout first. If that layout cannot be selected, prints
# RESULT layout-unavailable and exits 0 without calling the shortcuts a pass.
set -u

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
out="${KEYSTROKE_OUT:-$root/docs/compare}"
mkdir -p "$out"
result="$out/keystroke-result.txt"

echo "RESULT pending" > "$result"

cat > /tmp/lux-select-spanish.swift << 'SWIFT'
import Carbon
import Foundation

func sourceId(_ source: TISInputSource) -> String? {
    guard let raw = TISGetInputSourceProperty(source, kTISPropertyInputSourceID) else {
        return nil
    }
    return Unmanaged<CFString>.fromOpaque(raw).takeUnretainedValue() as String
}

func sources(_ includeAll: Bool) -> [TISInputSource] {
    guard let list = TISCreateInputSourceList(nil, includeAll)?.takeRetainedValue() as? [TISInputSource] else {
        return []
    }
    return list
}

fputs("enabled input sources:\n", stderr)
for source in sources(true) {
    if let id = sourceId(source) {
        fputs("  \(id)\n", stderr)
    }
}

let wanted = [
    "com.apple.keylayout.Spanish-ISO",
    "com.apple.keylayout.Spanish",
]
for id in wanted {
    let query = [kTISPropertyInputSourceID as String: id] as CFDictionary
    guard let list = TISCreateInputSourceList(query, true)?.takeRetainedValue() as? [TISInputSource],
          let source = list.first else {
        continue
    }
    _ = TISEnableInputSource(source)
    let status = TISSelectInputSource(source)
    if status == noErr {
        print(id)
        exit(0)
    }
    fputs("select \(id) status \(status)\n", stderr)
}
exit(2)
SWIFT

selected="$(swift /tmp/lux-select-spanish.swift 2>"$out/input-sources.txt" || true)"
echo "--> selector stdout: ${selected:-<empty>}"
echo "--> input sources:"
cat "$out/input-sources.txt" || true
if [[ "$selected" != com.apple.keylayout.Spanish* ]]; then
    echo "RESULT layout-unavailable" | tee "$result"
    echo "The macos runner did not select a Spanish layout. Shortcuts were not pressed."
    echo "Run the same script on a Mac where Spanish - ISO is enabled."
    exit 0
fi
echo "--> Spanish layout selected: $selected"

echo "--> cargo build -p lux"
cargo build -p lux --locked
bin="$root/target/debug/lux"
note="$(mktemp -d)/note.txt"
printf 'alpha\n' > "$note"

"$bin" "$note" >/tmp/lux-keystroke.log 2>&1 &
lux_pid=$!
cleanup() {
    if kill -0 "$lux_pid" 2>/dev/null; then
        kill "$lux_pid" 2>/dev/null || true
        wait "$lux_pid" 2>/dev/null || true
    fi
}
trap cleanup EXIT

cat > /tmp/lux-find-window.swift << 'SWIFT'
import CoreGraphics
import Foundation
let want = CommandLine.arguments.dropFirst().first ?? ""
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let info = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else { exit(1) }
var best = ""
var bestArea = 0.0
for w in info {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    if layer != 0 || owner.caseInsensitiveCompare(want) != .orderedSame { continue }
    guard let bounds = w[kCGWindowBounds as String] as? [String: Any] else { continue }
    let width = (bounds["Width"] as? NSNumber)?.doubleValue ?? 0
    let height = (bounds["Height"] as? NSNumber)?.doubleValue ?? 0
    if width < 200 || height < 200 { continue }
    let area = width * height
    if area <= bestArea { continue }
    bestArea = area
    let id = (w[kCGWindowNumber as String] as? NSNumber)?.intValue ?? 0
    let x = (bounds["X"] as? NSNumber)?.doubleValue ?? 0
    let y = (bounds["Y"] as? NSNumber)?.doubleValue ?? 0
    best = "\(id) \(Int(x)) \(Int(y)) \(Int(width)) \(Int(height))"
}
if best.isEmpty { exit(1) }
print(best)
SWIFT

found=""
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
    if found="$(swift /tmp/lux-find-window.swift lux 2>/dev/null)"; then
        break
    fi
    sleep 1
done
if [[ -z "$found" ]]; then
    echo "RESULT lux-window-missing" | tee "$result"
    echo "Lux did not open a window. Log:"
    cat /tmp/lux-keystroke.log || true
    exit 1
fi
echo "--> lux window $found"
window_id="${found%% *}"
osascript -e 'tell application "System Events" to set frontmost of process "lux" to true' || true
sleep 0.4

# Physical ANSI positions. Command+F opens find. Backslash is ç on Spanish ISO.
cat > /tmp/lux-type-cedilla.swift << 'SWIFT'
import CoreGraphics
import Foundation

func post(_ key: CGKeyCode, _ down: Bool, _ flags: CGEventFlags) {
    let source = CGEventSource(stateID: .hidSystemState)
    guard let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: down) else {
        exit(1)
    }
    event.flags = flags
    event.post(tap: .cghidEventTap)
}
func tap(_ key: CGKeyCode, _ flags: CGEventFlags) {
    post(key, true, flags)
    post(key, false, flags)
    Thread.sleep(forTimeInterval: 0.08)
}
tap(0x03, .maskCommand)
Thread.sleep(forTimeInterval: 0.35)
tap(0x2A, [])
SWIFT
if ! swift /tmp/lux-type-cedilla.swift; then
    echo "RESULT events-failed" | tee "$result"
    exit 1
fi
sleep 0.4
screencapture -l "$window_id" -o "$out/keystrokes-search.png" || true

cat > /tmp/lux-read-ax.swift << 'SWIFT'
import ApplicationServices
import Foundation

let pid = pid_t(CommandLine.arguments[1])!
let needle = CommandLine.arguments.count > 2 ? CommandLine.arguments[2] : "ç"
let app = AXUIElementCreateApplication(pid)

func text(_ element: AXUIElement, _ name: String) -> String? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else {
        return nil
    }
    return value as? String
}

func children(_ element: AXUIElement) -> [AXUIElement] {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &value) == .success,
          let list = value as? [AXUIElement] else {
        return []
    }
    return list
}

func walk(_ element: AXUIElement, _ depth: Int, _ lines: inout [String]) {
    if depth > 12 { return }
    let role = text(element, kAXRoleAttribute as String) ?? "?"
    if let value = text(element, kAXValueAttribute as String), !value.isEmpty {
        lines.append("\(role) \(value)")
    }
    for child in children(element) {
        walk(child, depth + 1, &lines)
    }
}

var dumped = ""
for _ in 0..<12 {
    var lines: [String] = []
    var focused: CFTypeRef?
    if AXUIElementCopyAttributeValue(app, kAXFocusedUIElementAttribute as CFString, &focused) == .success,
       let focused {
        let element = focused as! AXUIElement
        if let value = text(element, kAXValueAttribute as String) {
            lines.append("focused \(value)")
        }
    }
    walk(app, 0, &lines)
    dumped = lines.joined(separator: "\n")
    if lines.contains(where: { $0.contains(needle) }) {
        print(dumped)
        exit(0)
    }
    Thread.sleep(forTimeInterval: 0.25)
}
fputs(dumped.isEmpty ? "AX tree had no values\n" : dumped + "\n", stderr)
exit(3)
SWIFT

observed="$(swift /tmp/lux-read-ax.swift "$lux_pid" 2>"$out/ax-error.txt" || true)"
echo "--> AX value: ${observed:-<empty>}"
cat "$out/ax-error.txt" || true
if [[ "$observed" != *ç* ]]; then
    echo "RESULT assertions-failed" | tee "$result"
    echo "Spanish layout was active, but the focused field did not contain ç."
    echo "This is not a pass."
    exit 1
fi

# Further physical chords, still with Spanish selected. The ç check above
# is the assertion. These presses are captured; they are not extra passes.
cat > /tmp/lux-more-chords.swift << 'SWIFT'
import CoreGraphics
import Foundation
func post(_ key: CGKeyCode, _ down: Bool, _ flags: CGEventFlags) {
    let source = CGEventSource(stateID: .hidSystemState)
    guard let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: down) else { exit(1) }
    event.flags = flags
    event.post(tap: .cghidEventTap)
}
func tap(_ key: CGKeyCode, _ flags: CGEventFlags) {
    post(key, true, flags)
    post(key, false, flags)
    Thread.sleep(forTimeInterval: 0.08)
}
let cmd = CGEventFlags.maskCommand
let opt = CGEventFlags.maskAlternate
let shift = CGEventFlags.maskShift
tap(0x08, [cmd, opt])                 // Option+Command+C, search case
tap(0x35, [])                         // Escape
Thread.sleep(forTimeInterval: 0.2)
tap(0x23, [cmd, shift])               // Command+Shift+P
Thread.sleep(forTimeInterval: 0.25)
tap(0x35, [])
tap(0x2C, cmd)                        // Command+/
tap(0x21, [cmd, opt])                 // Option+Command+[
tap(0x1E, [cmd, opt])                 // Option+Command+]
tap(0x03, [cmd, opt])                 // Option+Command+F
tap(0x01, [cmd, opt])                 // Option+Command+S
tap(0x08, [cmd, opt])                 // Option+Command+C
tap(0x35, [])
SWIFT
swift /tmp/lux-more-chords.swift || true
sleep 0.3
screencapture -l "$window_id" -o "$out/keystrokes-chords.png" || true
echo "RESULT assertions-passed" | tee "$result"
echo "The focused field contained ç after a physical key press on the Spanish layout."
