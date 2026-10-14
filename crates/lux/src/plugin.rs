//! Sandboxed WASM plugin host integration (Goal G18).
//! Powered by `lux-plugin` (wasmtime Component Model, fuel limits, permission grants).

#[cfg(test)]
mod tests;

use std::fs;
use std::path::{Path, PathBuf};

use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, Key, NamedKey, PlatformContext};
use lux_plugin::{
    Capability, HostError, InstallError, InstalledPlugin, PermissionSet, PluginHost,
    install_from_dir,
};

use crate::app::App;

/// Pending capability grant request awaiting explicit user confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingGrant {
    pub(crate) plugin_id: String,
    pub(crate) command: String,
    pub(crate) capabilities: Vec<Capability>,
}

/// State for installed plugins, runtime execution, and permission grants.
pub(crate) struct PluginState {
    pub(crate) plugins_dir: PathBuf,
    pub(crate) installed: Vec<InstalledPlugin>,
    pub(crate) pending_grant: Option<PendingGrant>,
    pub(crate) last_output: Option<String>,
}

impl Default for PluginState {
    fn default() -> Self {
        let plugins_dir = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".lux")
            .join("plugins");
        Self::new(plugins_dir)
    }
}

impl PluginState {
    pub(crate) fn new(plugins_dir: PathBuf) -> Self {
        let mut state = Self {
            plugins_dir,
            installed: Vec::new(),
            pending_grant: None,
            last_output: None,
        };
        state.discover();
        state
    }

    /// Discovers all locally installed plugins inside `plugins_dir`.
    pub(crate) fn discover(&mut self) {
        self.installed.clear();
        let Ok(entries) = fs::read_dir(&self.plugins_dir) else {
            return;
        };
        for entry in entries.flatten() {
            let manifest_path = entry.path().join("lux-plugin.json");
            if manifest_path.exists()
                && let Ok(bytes) = fs::read(&manifest_path)
                && let Ok(manifest) = serde_json::from_slice::<serde_json::Value>(&bytes)
            {
                let id = manifest["id"].as_str().unwrap_or("").to_string();
                let version = manifest["version"].as_u64().unwrap_or(1) as u32;
                let wasm_name = manifest["wasm"].as_str().unwrap_or("plugin.wasm");
                let wasm_path = entry.path().join(wasm_name);
                let commands = manifest["commands"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let themes = manifest["themes"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                if !id.is_empty() {
                    self.installed.push(InstalledPlugin {
                        id,
                        version,
                        wasm_path,
                        commands,
                        grammars: Vec::new(),
                        themes,
                        formatters: Vec::new(),
                        lsp_servers: Vec::new(),
                        decorations: Vec::new(),
                    });
                }
            }
        }
    }

    /// Installs a plugin directory into `plugins_dir`.
    pub(crate) fn install(&mut self, src: &Path) -> Result<InstalledPlugin, InstallError> {
        let installed = install_from_dir(src, &self.plugins_dir)?;
        self.installed.retain(|p| p.id != installed.id);
        self.installed.push(installed.clone());
        Ok(installed)
    }

    /// Looks up an installed plugin by id.
    pub(crate) fn get(&self, id: &str) -> Option<&InstalledPlugin> {
        self.installed.iter().find(|p| p.id == id)
    }

    /// Requests running a plugin command. If capabilities are already granted,
    /// runs immediately and returns `Ok(Some(result))`. If capabilities are missing,
    /// sets `pending_grant` and returns `Ok(None)`.
    pub(crate) fn request_run(
        &mut self,
        plugin_id: &str,
        command: &str,
        required_caps: &[Capability],
        workspace: Option<&Path>,
    ) -> Result<Option<String>, HostError> {
        let plugin = self.get(plugin_id).cloned().ok_or_else(|| {
            HostError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "plugin not found",
            ))
        })?;

        let grant_file = workspace.map(|w| PermissionSet::workspace_file(w, plugin_id));
        let grants = grant_file
            .as_deref()
            .and_then(|f| PermissionSet::load(f).ok())
            .unwrap_or_default();

        let ungranted: Vec<Capability> = required_caps
            .iter()
            .copied()
            .filter(|&c| !grants.allows(c))
            .collect();

        if !ungranted.is_empty() {
            self.pending_grant = Some(PendingGrant {
                plugin_id: plugin_id.to_string(),
                command: command.to_string(),
                capabilities: ungranted,
            });
            return Ok(None);
        }

        let host = PluginHost::new(grants)?
            .with_fuel(1_000_000)
            .with_max_memory(4 * 1024 * 1024);

        let output = host.run_file(&plugin.wasm_path)?;
        self.last_output = Some(output.clone());
        Ok(Some(output))
    }

    /// Confirms the currently pending grant, saves it, and executes the pending command.
    pub(crate) fn confirm_grant(
        &mut self,
        workspace: Option<&Path>,
    ) -> Result<Option<String>, HostError> {
        let Some(pending) = self.pending_grant.take() else {
            return Ok(None);
        };

        if let Some(w) = workspace {
            let grant_file = PermissionSet::workspace_file(w, &pending.plugin_id);
            let mut grants = PermissionSet::load(&grant_file).unwrap_or_default();
            for cap in &pending.capabilities {
                grants.grant(*cap);
            }
            let _ = grants.save(&grant_file);
        }

        self.request_run(&pending.plugin_id, &pending.command, &[], workspace)
    }

    /// Denies the pending capability grant request.
    pub(crate) fn deny_grant(&mut self) {
        self.pending_grant = None;
    }
}

pub(crate) fn handle_plugin_key(
    app: &mut App,
    _ctx: &dyn PlatformContext,
    event: &KeyEvent,
) -> bool {
    if event.state != ElementState::Pressed {
        return false;
    }
    if app.ui.plugin.pending_grant.is_none() {
        return false;
    }
    match &event.key {
        Key::Named(NamedKey::Enter) => {
            let ws = app.workspace_root();
            let _ = app.ui.plugin.confirm_grant(ws.as_deref());
            app.scheduler.invalidate_all();
            if let Some(gpu) = &app.gpu_window {
                gpu.window.request_redraw();
            }
            true
        }
        Key::Named(NamedKey::Escape) => {
            app.ui.plugin.deny_grant();
            app.scheduler.invalidate_all();
            if let Some(gpu) = &app.gpu_window {
                gpu.window.request_redraw();
            }
            true
        }
        _ => true,
    }
}

pub(crate) fn handle_plugin_click(app: &mut App, x: f32, y: f32) -> bool {
    if app.ui.plugin.pending_grant.is_none() {
        return false;
    }
    let scale = app.scale_factor;
    let window_w = app.viewport.width;
    let window_h = app.viewport.height + app.tab_bar_height() + app.status_bar_height();
    let card_w = 460.0 * scale;
    let card_h = 160.0 * scale;
    let card_x = ((window_w - card_w) * 0.5).max(0.0);
    let card_y = ((window_h - card_h) * 0.5).max(0.0);

    let btn_y = card_y + card_h - 44.0 * scale;
    let btn_h = 28.0 * scale;
    let grant_w = 140.0 * scale;
    let grant_x = card_x + 18.0 * scale;
    let deny_w = 90.0 * scale;
    let deny_x = grant_x + grant_w + 12.0 * scale;

    if y >= btn_y && y < btn_y + btn_h {
        if x >= grant_x && x < grant_x + grant_w {
            let ws = app.workspace_root();
            let _ = app.ui.plugin.confirm_grant(ws.as_deref());
            app.scheduler.invalidate_all();
            return true;
        } else if x >= deny_x && x < deny_x + deny_w {
            app.ui.plugin.deny_grant();
            app.scheduler.invalidate_all();
            return true;
        }
    }
    true
}
