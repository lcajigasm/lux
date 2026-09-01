//! Local crash files and Ed25519-verified updates. No network upload.

mod crash;
mod update;

pub use crash::{crash_dir, install_panic_hook, write_crash};
pub use update::{UpdateError, verify_update};
