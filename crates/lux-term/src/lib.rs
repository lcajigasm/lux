//! Real PTY-backed terminal emulation core. Desktop only — this capability
//! does not exist on iOS/iPadOS (no child processes there, ADR 0006).

mod links;
mod pty;
mod scrollback;
mod sessions;
mod tasks;
#[cfg(test)]
mod vt;

pub use alacritty_terminal::event::Event;
pub use links::{SourceLocation, detect_links};
pub use portable_pty::{CommandBuilder, ExitStatus, PtySize};
pub use pty::{Pty, PtyError};
pub use scrollback::{
    Column, Line, Match, Point, RegexSearch, Scroll, ScrollbackPosition, SearchDirection,
    SelectionType,
};
pub use sessions::{ShellProfile, TerminalId, Terminals};
pub use tasks::{TaskId, TaskPanel};
