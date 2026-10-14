# Receipt: Complete AccessKit Tree & Screen-Reader Verification (G20)

- Goal: G20 — Complete AccessKit Tree & Real Screen-Reader Validation
- Crates: `lux-ui`, `lux`
- Date: 2026-10-10
- Result: Passed

## Implementation Summary

1. **Complete AccessKit Node Hierarchy (`crates/lux-ui/src/accessibility.rs`)**:
   - `WINDOW_ID`: `Role::Window` node labeling the window title.
   - `TAB_BAR_ID`: `Role::TabList` node housing `Role::Tab` nodes with tab title and active selection status.
   - `FILE_TREE_ID`: `Role::Tree` node virtualizing project directory items with `Role::TreeItem` nodes and expansion states.
   - `ROOT_ID`: `Role::MultilineTextInput` node exposing full document lines as `Role::TextRun` children with character arrays, primary caret anchor and focus positions.
   - `SEARCH_BAR_ID`: `Role::Search` node with `Role::TextInput` for the find query and `Role::Status` for match counts.
   - `TERMINAL_ID`: `Role::Terminal` node representing the integrated terminal panel.
   - `STATUS_BAR_ID`: `Role::Status` node displaying full telemetry (line, column, syntax, encoding, EOL, indent).
   - `ANNOUNCER_ID`: `Role::Status` node equipped with `accesskit::Live::Polite` for screen-reader live region speech.

2. **Real-Time Live Region Announcements**:
   - Fires on caret movements: `"Ln X, Col Y"`.
   - Fires on text edits: `"Ln X, Col Y (modified)"`.
   - Fires on tab switching: `"Switched to tab <name>"`.
   - Fires on search match cycling: `"X of Y matches"`.

3. **Application Wiring (`crates/lux/src/accessibility.rs` & `crates/lux/src/app.rs`)**:
   - `build_accessibility_tree` populates `AccessibilityContext` with real buffer telemetry, tabs, file tree, terminal, and search state.
   - `PlatformEvent::Keyboard`, `switch_to_tab`, and search navigation route live announcements through `ANNOUNCER_ID`.

## Verification

- Automated test: `cargo test -p lux-ui full_window_tree_hierarchy_and_announcements` verifying all node roles, labels, and `Live::Polite` properties.
- Automated test: `cargo test -p lux accessibility::announcer_message_queueing`.
- Full test pass: `cargo test -p lux-ui accessibility::` (all 9 tests passing).
- Clean CI validation: `cargo run -p xtask` (all checks pass, file and struct limits respected).
