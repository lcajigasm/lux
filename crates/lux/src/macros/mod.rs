//! Sublime Text Keyboard Macro recording and playback subsystem (ST19).

pub mod command;
pub mod dispatch;
pub mod macro_def;
pub mod playback;
pub mod recording;

#[allow(unused_imports)]
pub use command::MacroCommand;
#[allow(unused_imports)]
pub use macro_def::Macro;
pub use recording::MacroState;
