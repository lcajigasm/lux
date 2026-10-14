use std::path::Path;
use tempfile::tempdir;

use crate::plugin::PluginState;
use lux_plugin::Capability;

#[test]
fn plugin_state_empty_dir() {
    let tmp = tempdir().unwrap();
    let state = PluginState::new(tmp.path().to_path_buf());
    assert!(state.installed.is_empty());
    assert!(state.pending_grant.is_none());
    assert!(state.last_output.is_none());
}

#[test]
fn install_and_discover_plugins() {
    let tmp = tempdir().unwrap();
    let mut state = PluginState::new(tmp.path().to_path_buf());

    let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("lux-plugin")
        .join("examples");

    // Install echo plugin
    let installed = state.install(&examples_dir.join("echo")).unwrap();
    assert_eq!(installed.id, "echo");
    assert_eq!(installed.version, 1);
    assert_eq!(installed.commands, vec!["echo"]);

    // Install theme plugin
    let theme_installed = state
        .install(&examples_dir.join("theme-lux-dark-plus"))
        .unwrap();
    assert_eq!(theme_installed.id, "lux-dark-plus");
    assert_eq!(theme_installed.themes, vec!["theme.json"]);

    // Install identity formatter
    let fmt_installed = state.install(&examples_dir.join("identity-fmt")).unwrap();
    assert_eq!(fmt_installed.id, "identity-fmt");

    // Verify lookup
    assert!(state.get("echo").is_some());
    assert!(state.get("lux-dark-plus").is_some());
    assert!(state.get("identity-fmt").is_some());
    assert!(state.get("nonexistent").is_none());

    // Verify rediscovery from disk
    let reloaded = PluginState::new(tmp.path().to_path_buf());
    assert_eq!(reloaded.installed.len(), 3);
    assert!(reloaded.get("echo").is_some());
}

#[test]
fn capability_grant_flow() {
    let tmp = tempdir().unwrap();
    let ws = tempdir().unwrap();
    let mut state = PluginState::new(tmp.path().to_path_buf());

    let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("lux-plugin")
        .join("examples");
    state.install(&examples_dir.join("echo")).unwrap();

    // 1. Request run requires FileRead capability
    let res = state
        .request_run("echo", "echo", &[Capability::FileRead], Some(ws.path()))
        .unwrap();

    // Not yet granted -> returns None, pending_grant is set
    assert!(res.is_none());
    assert!(state.pending_grant.is_some());
    let pending = state.pending_grant.as_ref().unwrap();
    assert_eq!(pending.plugin_id, "echo");
    assert_eq!(pending.capabilities, vec![Capability::FileRead]);

    // 2. Deny grant clears pending
    state.deny_grant();
    assert!(state.pending_grant.is_none());

    // 3. Request again, then confirm grant
    let _ = state
        .request_run("echo", "echo", &[Capability::FileRead], Some(ws.path()))
        .unwrap();
    assert!(state.pending_grant.is_some());

    let run_res = state.confirm_grant(Some(ws.path())).unwrap();
    assert!(run_res.is_some());
    assert!(state.pending_grant.is_none());
    assert_eq!(state.last_output.as_deref(), Some("100"));

    // 4. Subsequent run doesn't prompt again (grants persisted in workspace)
    let next_run = state
        .request_run("echo", "echo", &[Capability::FileRead], Some(ws.path()))
        .unwrap();
    assert!(next_run.is_some());
    assert!(state.pending_grant.is_none());
}

#[test]
fn zero_capability_plugin_runs_without_prompt() {
    let tmp = tempdir().unwrap();
    let mut state = PluginState::new(tmp.path().to_path_buf());

    let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("lux-plugin")
        .join("examples");
    state.install(&examples_dir.join("identity-fmt")).unwrap();

    let res = state
        .request_run("identity-fmt", "identity", &[], None)
        .unwrap();
    assert!(res.is_some());
    assert!(state.pending_grant.is_none());
    assert_eq!(state.last_output.as_deref(), Some("200"));
}
