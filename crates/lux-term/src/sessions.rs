//! Named terminal sessions and shell profiles. Each session owns a real
//! `Pty`; killing one does not touch the others.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::pty::{CommandBuilder, ExitStatus, Pty, PtyError, PtySize};

/// How to spawn a shell: program, args, optional cwd and env overlays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellProfile {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
}

impl ShellProfile {
    /// Login-ish default: `$SHELL` on Unix, `cmd.exe` on Windows.
    pub fn system_default() -> Self {
        #[cfg(windows)]
        {
            Self {
                name: "cmd".into(),
                program: "cmd.exe".into(),
                args: Vec::new(),
                cwd: None,
                env: Vec::new(),
            }
        }
        #[cfg(not(windows))]
        {
            let program = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
            Self {
                name: "shell".into(),
                program,
                args: Vec::new(),
                cwd: None,
                env: Vec::new(),
            }
        }
    }

    pub fn command(&self) -> CommandBuilder {
        let mut cmd = CommandBuilder::new(&self.program);
        if !self.args.is_empty() {
            cmd.args(&self.args);
        }
        if let Some(cwd) = &self.cwd {
            cmd.cwd(cwd);
        }
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        cmd
    }
}

/// Stable handle for one session inside a [`Terminals`] set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalId(u64);

impl TerminalId {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Several live PTY sessions, addressed by id and a unique display name.
pub struct Terminals {
    next_id: u64,
    sessions: BTreeMap<TerminalId, NamedPty>,
}

struct NamedPty {
    name: String,
    pty: Pty,
}

impl Default for Terminals {
    fn default() -> Self {
        Self::new()
    }
}

impl Terminals {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            sessions: BTreeMap::new(),
        }
    }

    /// Spawn `profile` as a new session. `name` must be unique among live
    /// sessions.
    pub fn spawn(
        &mut self,
        name: impl Into<String>,
        profile: &ShellProfile,
        size: PtySize,
    ) -> Result<TerminalId, PtyError> {
        let name = name.into();
        if self.sessions.values().any(|s| s.name == name) {
            return Err(PtyError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("terminal name {name:?} already in use"),
            )));
        }
        let pty = Pty::spawn(profile.command(), size)?;
        let id = TerminalId(self.next_id);
        self.next_id += 1;
        self.sessions.insert(id, NamedPty { name, pty });
        Ok(id)
    }

    pub fn get(&self, id: TerminalId) -> Option<&Pty> {
        self.sessions.get(&id).map(|s| &s.pty)
    }

    pub fn get_mut(&mut self, id: TerminalId) -> Option<&mut Pty> {
        self.sessions.get_mut(&id).map(|s| &mut s.pty)
    }

    pub fn name(&self, id: TerminalId) -> Option<&str> {
        self.sessions.get(&id).map(|s| s.name.as_str())
    }

    pub fn ids(&self) -> impl Iterator<Item = TerminalId> + '_ {
        self.sessions.keys().copied()
    }

    /// Kill and reap one session. Other sessions keep running.
    pub fn kill(&mut self, id: TerminalId) -> Result<ExitStatus, PtyError> {
        let session = self.sessions.remove(&id).ok_or_else(|| {
            PtyError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no such terminal",
            ))
        })?;
        session.pty.kill_and_wait()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn small() -> PtySize {
        PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }
    }

    fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if condition() {
                return true;
            }
            if Instant::now() >= deadline {
                return condition();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn two_named_shells_kill_one_leaves_the_other() {
        let mut profile = ShellProfile::system_default();
        profile.program = "/bin/sh".into();
        profile.args = vec!["-c".into(), "sleep 30".into()];

        let mut terminals = Terminals::new();
        let a = terminals
            .spawn("build", &profile, small())
            .expect("spawn build");
        let b = terminals
            .spawn("repl", &profile, small())
            .expect("spawn repl");
        assert_eq!(terminals.name(a), Some("build"));
        assert_eq!(terminals.name(b), Some("repl"));

        let pid_b = terminals.get(b).and_then(Pty::pid).expect("repl pid");
        terminals.kill(a).expect("kill build");
        assert!(terminals.get(a).is_none());
        assert!(terminals.get(b).is_some());

        let rc = unsafe { libc::kill(pid_b as libc::pid_t, 0) };
        assert_eq!(rc, 0, "surviving session must still be a live process");

        terminals.kill(b).expect("kill repl");
    }

    #[test]
    fn profile_cwd_and_env_reach_the_child() {
        let dir = std::env::temp_dir();
        let profile = ShellProfile {
            name: "pwd".into(),
            program: "/bin/sh".into(),
            args: vec![
                "-c".into(),
                "printf \"%s %s\" \"$PWD\" \"$LUX_TERM_MARK\"".into(),
            ],
            cwd: Some(dir.clone()),
            env: vec![("LUX_TERM_MARK".into(), "from-profile".into())],
        };
        // `$PWD` may be a symlink-resolved path; compare via `canonicalize`.
        let expected_cwd = dir.canonicalize().unwrap_or(dir);

        let mut terminals = Terminals::new();
        let id = terminals.spawn("pwd", &profile, small()).expect("spawn");
        let pty = terminals.get_mut(id).expect("session");
        assert!(
            wait_until(Duration::from_secs(5), || {
                let snap = pty.snapshot();
                snap.contains("from-profile")
                    && snap.contains(expected_cwd.to_string_lossy().as_ref())
            }),
            "profile cwd/env missing:\n{}",
            pty.snapshot()
        );
        let _ = terminals.kill(id);
    }

    #[test]
    fn duplicate_name_is_rejected() {
        let mut profile = ShellProfile::system_default();
        profile.program = "/bin/sh".into();
        profile.args = vec!["-c".into(), "sleep 30".into()];
        let mut terminals = Terminals::new();
        terminals.spawn("one", &profile, small()).expect("first");
        assert!(terminals.spawn("one", &profile, small()).is_err());
        let id = terminals.ids().next().expect("id");
        terminals.kill(id).expect("cleanup");
    }
}
