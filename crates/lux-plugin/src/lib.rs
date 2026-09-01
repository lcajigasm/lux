//! Capability-scoped WASM plugin host (ADR 0016).

mod host;
mod install;
mod permissions;

pub use host::{HostError, PluginHost};
pub use install::{InstallError, InstalledPlugin, install_from_dir};
pub use permissions::{Capability, GrantError, PermissionSet};
