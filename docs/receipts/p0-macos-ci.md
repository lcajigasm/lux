# Receipt: P0 macOS checklist

- Phase: P0
- Crate(s): lux
- Goal: the parity checklist runs on macos-15 against Lux.app
- Verify: scripts/p0-checklist.sh
- Result: passed
- Run: https://github.com/lcajigasm/lux/actions/runs/43152259652
- Opened: 2026-11-12
- Closed: 2026-11-12

| Check | Status | Evidence |
|---|---|---|
| Tabs | covered | probe tabs=2 |
| Scroll | covered | scroll_y 0 -> 146 (wheel) |
| Native menu bar | covered | menu bar item: Lux |
| Cmd+P | covered | goto panel opened |
| Cmd+Shift+P | covered | palette opened |
| Open a folder | covered | sidebar=1 for /var/folders/2s/vty41l455yv698g4xkgfv7z00000gn/T/tmp.pCC3vEhcVh/folder |
| System monospace | covered | Lux: using system font /System/Library/Fonts/SFNSMono.ttf |
| Shortcut matrix | covered | The shortcuts job presses the Block 3 chords and checks the file or the probe. |
| Multi-cursor | covered | The shortcuts job types through Cmd+D and checks the saved file. |
| Retina window | see E1 | p0-hidpi.sh records whether the real framebuffer can be scale 2. |
| Minimap | not this job | The parked minimap change is untouched. |
| Light and dark appearance | solo-Mac | Switching the runner appearance and reading the window pixels is not automated here. |
| Traffic lights | solo-Mac | The job checks that a Lux window exists. It does not click the close, minimize, or zoom buttons. |
| Finder Open With | solo-Mac | The runner has no Finder handoff to assert. |

A row marked solo-Mac was not given a pass. The job fails only when a row marked failed is present.
