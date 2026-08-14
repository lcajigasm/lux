//! Real PTY-backed terminal emulation core. Desktop only — this capability
//! does not exist on iOS/iPadOS (no child processes there, ADR 0006).

mod pty;

pub use alacritty_terminal::event::Event;
pub use portable_pty::{CommandBuilder, ExitStatus, PtySize};
pub use pty::{Pty, PtyError};
