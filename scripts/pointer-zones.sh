#!/usr/bin/env bash
# Moves the pointer across a real Lux.app window and reads
# NSCursor.currentSystem for each zone.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
# shellcheck source=macos-drive.sh
source "$root/scripts/macos-drive.sh"

real_home="${HOME}"
export HOME
HOME="$(mktemp -d)"
unset XDG_CACHE_HOME || true

out="$root/docs/evidence/pointer-zones"
mkdir -p "$out"
rm -f "$out"/*.png "$out"/pointer-zones.txt

HOME="$real_home" "$root/scripts/stage-lux-app.sh"
drive_init "$root/target/Lux.app"
trap drive_stop EXIT

dir="$(mktemp -d)"
note="$dir/note.txt"
md="$dir/page.md"
html="$dir/page.html"
printf 'hello\n' > "$note"
cat > "$md" << 'EOF'
# Title

See [Lux](https://example.com).
EOF
cat > "$html" << 'EOF'
<h1>Title</h1>
<p>See <a href="https://example.com">Lux</a>.</p>
EOF

cat > /tmp/lux-window-id.swift << 'SWIFT'
import CoreGraphics
import Foundation
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let info = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else { exit(1) }
for w in info {
    let owner = w[kCGWindowOwnerName as String] as? String ?? ""
    let layer = w[kCGWindowLayer as String] as? Int ?? 0
    if layer != 0 { continue }
    if owner.caseInsensitiveCompare("Lux") != .orderedSame && owner.caseInsensitiveCompare("lux") != .orderedSame {
        continue
    }
    guard let bounds = w[kCGWindowBounds as String] as? [String: Any] else { continue }
    let width = (bounds["Width"] as? NSNumber)?.doubleValue ?? 0
    let height = (bounds["Height"] as? NSNumber)?.doubleValue ?? 0
    if width < 200 || height < 200 { continue }
    let id = w[kCGWindowNumber as String] as? Int ?? 0
    print(id)
    exit(0)
}
exit(1)
SWIFT

cat > /tmp/lux-cursor-name.swift << 'SWIFT'
import AppKit
import Foundation

func tiff(_ cursor: NSCursor) -> Data {
    cursor.image.tiffRepresentation ?? Data()
}

func near(_ a: CGFloat, _ b: CGFloat) -> Bool {
    abs(a - b) < 0.51
}

func sameShape(_ a: NSCursor, _ b: NSCursor) -> Bool {
    near(a.hotSpot.x, b.hotSpot.x) && near(a.hotSpot.y, b.hotSpot.y)
        && near(a.image.size.width, b.image.size.width)
        && near(a.image.size.height, b.image.size.height)
}

func describe(_ cursor: NSCursor, _ name: String) -> String {
    let spot = cursor.hotSpot
    let size = cursor.image.size
    let rep = cursor.image.representations.first
    let px = rep.map { "\($0.pixelsWide)x\($0.pixelsHigh)" } ?? "none"
    return "\(name) \(spot.x),\(spot.y) \(size.width)x\(size.height) px=\(px) tiff=\(tiff(cursor).count)"
}

// Factory `image.size` is still 0 while the cursor is being matched, then
// becomes 32x32 or 24x24 once a representation is drawn. TIFF bytes also
// miss: run 44421066716 showed the live hand at 13,8 32x32 and both resize
// cursors at 12,12 24x24, and neither Data equality nor sameShape named them.
// The live hotspot and size are enough for the hand, the arrow and the I-beam.
// The two resize cursors share 12,12 24x24, so the ink axis splits them.
func nameByMeasure(_ cursor: NSCursor) -> String? {
    let spot = cursor.hotSpot
    let size = cursor.image.size
    if near(spot.x, 13) && near(spot.y, 8)
        && near(size.width, 32) && near(size.height, 32) {
        return "hand"
    }
    if near(spot.x, 4) && near(spot.y, 9)
        && near(size.width, 9) && near(size.height, 18) {
        return "ibeam"
    }
    if near(spot.x, 4) && near(spot.y, 4)
        && near(size.width, 17) && near(size.height, 23) {
        return "arrow"
    }
    if near(spot.x, 12) && near(spot.y, 12)
        && near(size.width, 24) && near(size.height, 24) {
        return resizeAxis(cursor)
    }
    return nil
}

func resizeAxis(_ cursor: NSCursor) -> String? {
    guard let tiff = cursor.image.tiffRepresentation,
        let rep = NSBitmapImageRep(data: tiff),
        let raw = rep.bitmapData
    else {
        return nil
    }
    let width = rep.pixelsWide
    let height = rep.pixelsHigh
    let samples = rep.samplesPerPixel
    guard width > 2, height > 2, samples > 0, rep.bitsPerSample == 8 else {
        return nil
    }
    let stride = rep.bytesPerRow
    let alpha = rep.hasAlpha ? samples - 1 : -1
    var minX = width
    var maxX = -1
    var minY = height
    var maxY = -1
    var rowInk = 0
    var colInk = 0
    let midX = width / 2
    let midY = height / 2
    for y in 0..<height {
        for x in 0..<width {
            let index = y * stride + x * samples
            let ink: Int
            if alpha >= 0 {
                ink = Int(raw[index + alpha])
            } else {
                var peak = 0
                for sample in 0..<samples {
                    peak = max(peak, Int(raw[index + sample]))
                }
                ink = peak
            }
            if ink <= 40 {
                continue
            }
            minX = min(minX, x)
            maxX = max(maxX, x)
            minY = min(minY, y)
            maxY = max(maxY, y)
            if abs(y - midY) <= 1 {
                rowInk += ink
            }
            if abs(x - midX) <= 1 {
                colInk += ink
            }
        }
    }
    if maxX < 0 {
        return nil
    }
    let spanX = maxX - minX
    let spanY = maxY - minY
    if spanX > spanY + 1 {
        return "ew"
    }
    if spanY > spanX + 1 {
        return "ns"
    }
    if rowInk > colInk {
        return "ew"
    }
    if colInk > rowInk {
        return "ns"
    }
    return nil
}

guard let current = NSCursor.currentSystem else {
    print("nil")
    exit(0)
}
if let named = nameByMeasure(current) {
    print(named)
    exit(0)
}
let known: [(NSCursor, String)] = [
    (.arrow, "arrow"),
    (.iBeam, "ibeam"),
    (.pointingHand, "hand"),
    (.resizeLeftRight, "ew"),
    (.resizeUpDown, "ns"),
]
// `current == cursor` is true for every factory cursor on macos-15, so it
// cannot name anything. Run 44410111912: the live I-beam is 4,9 9x18 while
// NSCursor.iBeam.image is empty; the live hand is the factory hand
// (13,8 32x32) but the broken equality put every name in the hit list.
// East-west and north-south share 12,12 24x24, so size is not enough.
var tiffHits: [String] = []
var shapeHits: [String] = []
for (cursor, name) in known {
    let data = tiff(cursor)
    if !data.isEmpty && tiff(current) == data {
        tiffHits.append(name)
    }
    if !data.isEmpty && sameShape(current, cursor) {
        shapeHits.append(name)
    }
}
if tiffHits.count == 1 {
    print(tiffHits[0])
    exit(0)
}
if shapeHits.count == 1 {
    print(shapeHits[0])
    exit(0)
}
// macos-15 factory images for arrow and I-beam are empty (hotspot 0,0
// and 0x0). The live arrow is 4,4 17x23 (run 44093132195). The live
// I-beam is 4,9 9x18 (run 44410111912).
let arrowImage = tiff(NSCursor.arrow)
if arrowImage.isEmpty
    && near(current.hotSpot.x, 4) && near(current.hotSpot.y, 4)
    && near(current.image.size.width, 17) && near(current.image.size.height, 23) {
    print("arrow")
    exit(0)
}
let beamImage = tiff(NSCursor.iBeam)
if beamImage.isEmpty
    && near(current.hotSpot.x, 4) && near(current.hotSpot.y, 9)
    && near(current.image.size.width, 9) && near(current.image.size.height, 18) {
    print("ibeam")
    exit(0)
}
let catalog = known.map { describe($0.0, $0.1) }.joined(separator: " | ")
let spot = current.hotSpot
let size = current.image.size
print("other \(spot.x),\(spot.y) \(size.width)x\(size.height) \(catalog)")
SWIFT

cat > /tmp/lux-screen-scale.swift << 'SWIFT'
import AppKit
let scale = NSScreen.main?.backingScaleFactor ?? 2
print(scale)
SWIFT

drive_stop
rm -f "$DRIVE_PROBE"
: > "$DRIVE_LOG"
"$DRIVE_APP/Contents/MacOS/lux" --probe "$DRIVE_PROBE" "$dir" "$note" "$md" "$html" >"$DRIVE_LOG" 2>&1 &
DRIVE_PID=$!
drive_wait_window
sleep 0.6
drive_focus

report="$out/pointer-zones.txt"
: > "$report"
fail=0

log() {
  printf '%s\n' "$*" | tee -a "$report"
}

frame() {
  swift /tmp/lux-find-window.swift "${DRIVE_OWNER:-Lux}"
}

num() {
  python3 -c "print(float('$1'))"
}

if ! swiftc -o /tmp/lux-cursor-name /tmp/lux-cursor-name.swift >"$out/cursor-build.txt" 2>&1; then
  log "cursor reader did not compile"
  cat "$out/cursor-build.txt" | tee -a "$report"
  exit 1
fi

read_cursor() {
  /tmp/lux-cursor-name
}

sample() {
  local zone="$1" expect="$2" x="$3" y="$4"
  drive_key move "$x" "$y"
  sleep 0.45
  local cursor pointer
  cursor="$(read_cursor || true)"
  drive_dump || true
  pointer="$(drive_field pointer || true)"
  log "zone=$zone expect=$expect nscursor=${cursor:-missing} pointer=${pointer:-absent} at=$x,$y"
  shot "$zone"
  if [[ "$cursor" != "$expect" ]]; then
    fail=1
  fi
}

shot() {
  local name="$1"
  local id
  id="$(swift /tmp/lux-window-id.swift || true)"
  if [[ -n "$id" ]]; then
    screencapture -C -l "$id" -o "$out/$name.png" || true
  fi
}

# The open queue leaves the last file active. note.txt is still a tab.
ready=0
for _ in $(seq 1 50); do
  if drive_dump; then
    tabs="$(drive_field tab_paths || true)"
    if [[ "$tabs" == *note.txt* && "$tabs" == *page.md* && "$tabs" == *page.html* ]]; then
      ready=1
      break
    fi
  fi
  sleep 0.3
done
if [[ "$ready" != 1 ]]; then
  log "the three files did not open: tabs=$(drive_field tab_paths || true) active=$(drive_field active_path || true)"
  cat "$DRIVE_LOG" >> "$report" || true
  exit 1
fi
log "open queue settled on $(drive_field active_path || true)"

# `sidebar` is 1 when the sidebar exists. Run 44401149823 read that flag
# as the width, clicked the header, and left page.html active. `sidebar_w`
# is the laid-out width in physical pixels. Wait until the tree has one.
sidebar_px="$(drive_field sidebar_w || true)"
wide() {
  python3 -c 'import sys; sys.exit(0 if float(sys.argv[1]) >= 120 else 1)' "${1:-0}"
}
if ! wide "${sidebar_px:-0}"; then
  for _ in $(seq 1 40); do
    sleep 0.1
    drive_dump || true
    sidebar_px="$(drive_field sidebar_w || true)"
    if wide "${sidebar_px:-0}"; then
      break
    fi
  done
fi

frame_line="$(frame)" || {
  log "Lux window was not found"
  exit 1
}
read -r frame_x frame_y frame_w frame_h <<< "$frame_line"
scale="$(swift /tmp/lux-screen-scale.swift)"
content="$(drive_field content || true)"
sidebar_px="$(drive_field sidebar_w || true)"
minimap_px="$(drive_field minimap || true)"
viewport="$(drive_field viewport || true)"
language="$(drive_field language_rect || true)"

log "frame=$frame_x,$frame_y ${frame_w}x${frame_h} screen_scale=$scale"
log "content=${content:-absent} sidebar_w=${sidebar_px:-absent} minimap=${minimap_px:-absent} viewport=${viewport:-absent}"
log "language_rect=${language:-absent}"

if [[ -n "$content" && -n "${sidebar_px:-}" ]]; then
  app_scale="$(drive_field scale)"
  IFS=',' read -r cx cy cw ch <<< "$content"
  origin_x="$(python3 -c "print($cx / $app_scale)")"
  origin_y="$(python3 -c "print($cy / $app_scale)")"
  content_w="$(python3 -c "print($cw / $app_scale)")"
  content_h="$(python3 -c "print($ch / $app_scale)")"
  sidebar="$(python3 -c "print($sidebar_px / $app_scale)")"
  minimap="$(python3 -c "print(${minimap_px:-0} / $app_scale)")"
  IFS=',' read -r _vw vh <<< "$viewport"
  # viewport width on 44401149823 was still the full window (1000) while
  # the minimap was already 90. The editor width is content minus chrome.
  view_w="$(python3 -c "print(max(1, $content_w - $sidebar - $minimap))")"
  view_h="$(python3 -c "print($vh / $app_scale)")"
else
  origin_x="$frame_x"
  origin_y="$(python3 -c "print($frame_y + 28)")"
  content_w="$frame_w"
  content_h="$(python3 -c "print($frame_h - 28)")"
  sidebar=220
  minimap=90
  view_w="$(python3 -c "print($content_w - $sidebar - $minimap)")"
  view_h="$(python3 -c "print($content_h - 32 - 24)")"
fi

pt() {
  python3 -c "print(round($origin_x + $1))"
}
py() {
  python3 -c "print(round($origin_y + $1))"
}

tab_w="$(python3 -c "print(min(200, max(120, $view_w / 3)))")"
log "origin=$origin_x,$origin_y content=${content_w}x${content_h} sidebar=$sidebar view=${view_w}x${view_h} tab_w=$tab_w"

tab_x="$(python3 -c "print($sidebar + 40)")"
drive_key click "$(pt "$tab_x")" "$(py 16)"
sleep 0.5
drive_dump || true
active="$(drive_field active_path || true)"
log "selected note tab active=$active"
if [[ "$active" != *note.txt ]]; then
  log "note.txt tab did not become active: ${active:-absent}"
  shot note-tab-miss
  exit 1
fi

text_x="$(python3 -c "print($sidebar + 140)")"
gutter_x="$(python3 -c "print($sidebar + 12)")"
sample editor-text ibeam "$(pt "$text_x")" "$(py 72)"

sample gutter arrow "$(pt "$gutter_x")" "$(py 72)"

close_x="$(python3 -c "print($sidebar + $tab_w - 12)")"
sample tab hand "$(pt "$tab_x")" "$(py 16)"
shot tab
sample tab-close hand "$(pt "$close_x")" "$(py 16)"

sample file-tree arrow "$(pt 40)" "$(py 76)"
sample sidebar-edge ew "$(pt "$sidebar")" "$(py 140)"
shot sidebar-edge

if [[ -n "$language" ]]; then
  IFS=',' read -r lx ly lw lh <<< "$language"
  app_scale="$(drive_field scale)"
  if python3 -c "import sys; sys.exit(0 if $lw > 1 else 1)"; then
    bx="$(python3 -c "print(round($origin_x + ($lx + $lw / 2) / $app_scale))")"
    by="$(python3 -c "print(round($origin_y + ($ly + $lh / 2) / $app_scale))")"
    sample status-button hand "$bx" "$by"
  else
    sample status-button hand "$(pt "$(python3 -c "print($content_w - 40)")")" "$(py "$(python3 -c "print($content_h - 12)")")"
  fi
else
  sample status-button hand "$(pt "$(python3 -c "print($content_w - 40)")")" "$(py "$(python3 -c "print($content_h - 12)")")"
fi

menu_x="$(pt "$text_x")"
menu_y="$(py 72)"
drive_key right "$menu_x" "$menu_y"
sleep 0.4
sample context-menu arrow "$(python3 -c "print($menu_x + 12)")" "$(python3 -c "print($menu_y + 20)")"
drive_key tap 53 ""
sleep 0.2

if (( frame_x > 50 )); then
  out_x=$((frame_x - 30))
else
  out_x=$((frame_x + frame_w + 30))
fi
sample outside arrow "$out_x" "$((frame_y + 8))"

drive_key tap 19 "cmd,alt"
sleep 0.4
div_x="$(python3 -c "print($sidebar + $view_w / 2)")"
div_y="$(python3 -c "print(32 + $view_h / 2)")"
sample column-divider ew "$(pt "$div_x")" "$(py "$div_y")"
shot column-divider

drive_key tap 19 "cmd,alt,shift"
sleep 0.4
sample row-divider ns "$(pt "$(python3 -c "print($sidebar + 80)")")" "$(py "$div_y")"

drive_key tap 18 "cmd,alt"
sleep 0.3

preview_at() {
  local kind="$1"
  local content_width pane_cap preview_w editor_w pane_x text_x heading_y paragraph_y pad_x
  local app_scale pane link_raw
  drive_dump || true
  app_scale="$(drive_field scale || echo 1)"
  pane="$(drive_field preview_pane || true)"
  link_raw="$(drive_field link_y || true)"
  if [[ "$pane" == *,* ]]; then
    pane_x="$(PANE="$pane" SCALE="$app_scale" python3 -c 'import os; p=os.environ["PANE"].split(","); print(float(p[0]) / float(os.environ["SCALE"]))')"
    preview_w="$(PANE="$pane" SCALE="$app_scale" python3 -c 'import os; p=os.environ["PANE"].split(","); print(float(p[1]) / float(os.environ["SCALE"]))')"
  else
    content_width="$(python3 -c "print($view_w - $sidebar)")"
    pane_cap="$(python3 -c "print(max(1, $content_width * 0.62))")"
    preview_w="$(python3 -c "print(min(max($content_width * 0.46, min(180, $pane_cap)), $pane_cap))")"
    editor_w="$(python3 -c "print(max(1, $content_width - $preview_w))")"
    pane_x="$(python3 -c "print($sidebar + $editor_w)")"
  fi
  text_x="$(python3 -c "print($pane_x + 24)")"
  heading_y="$(python3 -c "print(32 + 22.4 * 0.25 + 6)")"
  if [[ -n "$link_raw" ]]; then
    paragraph_y="$(LINK="$link_raw" SCALE="$app_scale" python3 -c 'import os; print(float(os.environ["LINK"]) / float(os.environ["SCALE"]))')"
  elif [[ "$kind" == html ]]; then
    paragraph_y="$(python3 -c "print(32 + 22.4 * 0.25 + 16 * 1.45 * 1.6 + 8)")"
  else
    paragraph_y="$(python3 -c "print(32 + 22.4 * 0.25 + 16 * 1.55 * 1.4 + 22.4 * 0.35 + 8)")"
  fi
  pad_x="$(python3 -c "print($pane_x + 4)")"
  log "$kind pane_x=$pane_x preview_w=$preview_w heading_y=$heading_y paragraph_y=$paragraph_y"
  sample "$kind-text" ibeam "$(pt "$text_x")" "$(py "$heading_y")"
  sample "$kind-padding" arrow "$(pt "$pad_x")" "$(py "$heading_y")"
  local found="" sx last=""
  local scan_y
  scan_y="$(py "$paragraph_y")"
  drive_key move "$(pt "$text_x")" "$scan_y"
  sleep 0.3
  drive_dump || true
  log "$kind-line pointer=$(drive_field pointer || true) at=$text_x,$paragraph_y"
  for step in $(seq 0 8 220); do
    sx="$(python3 -c "print($text_x + $step)")"
    drive_key move "$(pt "$sx")" "$scan_y"
    sleep 0.25
    local cursor
    cursor="$(read_cursor || true)"
    last="${cursor:-missing}"
    log "$kind-scan x=$sx nscursor=$last"
    if [[ "$cursor" == hand ]]; then
      found="$sx"
      break
    fi
  done
  if [[ -n "$found" ]]; then
    sample "$kind-link" hand "$(pt "$found")" "$scan_y"
    shot "$kind-link"
  else
    log "zone=$kind-link expect=hand nscursor=${last:-missing} pointer=absent at=scan"
    fail=1
  fi
}

md_tab="$(python3 -c "print($sidebar + $tab_w * 1.5)")"
drive_key click "$(pt "$md_tab")" "$(py 16)"
sleep 0.4
drive_dump || true
log "after markdown tab path=$(drive_field active_path || true) language=$(drive_field language || true)"
drive_key tap 9 "cmd,shift"
sleep 0.5
drive_dump || true
log "markdown preview=$(drive_field preview || true)"
preview_at markdown

html_tab="$(python3 -c "print($sidebar + $tab_w * 2.5)")"
drive_key click "$(pt "$html_tab")" "$(py 16)"
sleep 0.4
drive_dump || true
log "after html tab path=$(drive_field active_path || true) language=$(drive_field language || true)"
drive_key tap 9 "cmd,shift"
sleep 0.5
drive_dump || true
log "html preview=$(drive_field preview || true)"
preview_at html

log "result=$([[ "$fail" == 0 ]] && echo pass || echo fail)"
if [[ "$fail" != 0 ]]; then
  exit 1
fi
