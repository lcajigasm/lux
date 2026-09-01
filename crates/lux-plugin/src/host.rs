//! wasmtime Component Model host: fuel, memory limits, permission checks
//! before any host-side I/O.

use std::path::Path;

use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};

use crate::permissions::{Capability, PermissionSet};

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("{0}")]
    Wasm(#[from] wasmtime::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("capability {0:?} is not granted")]
    Denied(Capability),
    #[error("plugin export {0:?} missing")]
    MissingExport(&'static str),
    #[error("host has no HTTP/process backend")]
    Unimplemented,
}

struct Ctx {
    limits: StoreLimits,
}

pub struct PluginHost {
    engine: Engine,
    grants: PermissionSet,
    fuel: u64,
    max_memory: usize,
}

impl PluginHost {
    pub fn new(grants: PermissionSet) -> Result<Self, HostError> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.consume_fuel(true);
        Ok(Self {
            engine: Engine::new(&config)?,
            grants,
            fuel: 1_000_000,
            max_memory: 4 * 1024 * 1024,
        })
    }

    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self
    }

    pub fn with_max_memory(mut self, bytes: usize) -> Self {
        self.max_memory = bytes;
        self
    }

    pub fn grants(&self) -> &PermissionSet {
        &self.grants
    }

    fn require(&self, cap: Capability) -> Result<(), HostError> {
        if self.grants.allows(cap) {
            Ok(())
        } else {
            Err(HostError::Denied(cap))
        }
    }

    /// Host-side file read used by the file-read import. No grant → error,
    /// never bytes from disk.
    pub fn read_file(&self, path: &Path) -> Result<String, HostError> {
        self.require(Capability::FileRead)?;
        Ok(std::fs::read_to_string(path)?)
    }

    pub fn write_file(&self, path: &Path, body: &str) -> Result<(), HostError> {
        self.require(Capability::FileWrite)?;
        Ok(std::fs::write(path, body)?)
    }

    pub fn net_get(&self, _url: &str) -> Result<(), HostError> {
        self.require(Capability::Network)?;
        Err(HostError::Unimplemented)
    }

    pub fn proc_spawn(&self, _cmd: &str) -> Result<(), HostError> {
        self.require(Capability::Process)?;
        Err(HostError::Unimplemented)
    }

    /// Instantiate `wasm` and call the exported `run: func() -> string`.
    pub fn run_component(&self, wasm: &[u8]) -> Result<String, HostError> {
        let component = Component::from_binary(&self.engine, wasm)?;
        let linker = Linker::new(&self.engine);
        let limits = StoreLimitsBuilder::new()
            .memory_size(self.max_memory)
            .build();
        let mut store = Store::new(&self.engine, Ctx { limits });
        store.limiter(|ctx| &mut ctx.limits);
        store.set_fuel(self.fuel)?;
        let instance = linker.instantiate(&mut store, &component)?;
        let run = instance
            .get_typed_func::<(), (u32,)>(&mut store, "run")
            .map_err(|_| HostError::MissingExport("run"))?;
        let (out,) = run.call(&mut store, ())?;
        Ok(out.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component_run_hello() -> Vec<u8> {
        wat::parse_str(
            r#"
            (component
              (core module $m
                (func (export "run") (result i32) (i32.const 42)))
              (core instance $i (instantiate $m))
              (func (export "run") (result u32)
                (canon lift (core func $i "run")))
            )
            "#,
        )
        .expect("wat")
    }

    fn component_infinite_loop() -> Vec<u8> {
        wat::parse_str(
            r#"
            (component
              (core module $m
                (func (export "run") (result i32)
                  (loop $l (br $l))
                  (i32.const 0)))
              (core instance $i (instantiate $m))
              (func (export "run") (result u32)
                (canon lift (core func $i "run")))
            )
            "#,
        )
        .expect("wat")
    }

    fn component_huge_memory() -> Vec<u8> {
        wat::parse_str(
            r#"
            (component
              (core module $m
                (memory 200)
                (func (export "run") (result i32) (i32.const 1)))
              (core instance $i (instantiate $m))
              (func (export "run") (result u32)
                (canon lift (core func $i "run")))
            )
            "#,
        )
        .expect("wat")
    }

    #[test]
    fn runs_a_component_and_returns_string() {
        let host = PluginHost::new(PermissionSet::new([])).unwrap();
        assert_eq!(host.run_component(&component_run_hello()).unwrap(), "42");
    }

    #[test]
    fn fuel_stops_an_infinite_loop() {
        let host = PluginHost::new(PermissionSet::new([]))
            .unwrap()
            .with_fuel(1_000);
        assert!(host.run_component(&component_infinite_loop()).is_err());
    }

    #[test]
    fn memory_limit_rejects_a_200_page_module() {
        let host = PluginHost::new(PermissionSet::new([]))
            .unwrap()
            .with_max_memory(64 * 1024);
        assert!(host.run_component(&component_huge_memory()).is_err());
    }

    #[test]
    fn file_read_without_grant_does_not_touch_disk() {
        let dir = tempfile::tempdir().unwrap();
        let secret = dir.path().join("secret.txt");
        std::fs::write(&secret, "classified").unwrap();
        let host = PluginHost::new(PermissionSet::new([])).unwrap();
        assert!(matches!(
            host.read_file(&secret),
            Err(HostError::Denied(Capability::FileRead))
        ));
        assert_eq!(std::fs::read_to_string(&secret).unwrap(), "classified");
    }

    #[test]
    fn file_read_with_grant_returns_real_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("ok.txt");
        std::fs::write(&f, "ok").unwrap();
        let host = PluginHost::new(PermissionSet::new([Capability::FileRead])).unwrap();
        assert_eq!(host.read_file(&f).unwrap(), "ok");
    }

    #[test]
    fn network_and_process_denied_by_default() {
        let host = PluginHost::new(PermissionSet::new([])).unwrap();
        assert!(matches!(
            host.net_get("http://127.0.0.1"),
            Err(HostError::Denied(Capability::Network))
        ));
        assert!(matches!(
            host.proc_spawn("/bin/sh"),
            Err(HostError::Denied(Capability::Process))
        ));
    }
}
