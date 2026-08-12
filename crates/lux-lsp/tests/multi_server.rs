//! Phase 7's acceptance criteria name a specific bar beyond the nine
//! numbered deliverables: "funciona con al menos cinco servidores reales:
//! rust-analyzer, typescript-language-server, pyright, gopls y clangd."
//! Every other real-server test in this crate exercises only
//! `rust-analyzer`; this file is the one place that proves the client
//! isn't accidentally rust-analyzer-specific -- the same general
//! sequence (spawn, real capability handshake, `didOpen`, a real
//! `documentSymbol` request) run against a small, real, on-disk fixture
//! in each of the other four languages. `documentSymbol` (not hover) is
//! the shared probe: it doesn't need a pinpoint cursor position to land
//! on the right token, which keeps five differently-shaped fixtures
//! honest without five sets of fragile position math.
//!
//! Requires all five binaries reachable on `PATH`. Not run in CI by
//! default for that reason -- see `docs/receipts/phase-07-*.md` for how
//! this machine got them (`brew install gopls go`, `npm install -g
//! typescript-language-server typescript pyright`, `clangd` ships with
//! Xcode's command line tools).

mod servers {
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    use lux_lsp::{
        DocumentSync, Navigation, ServerRegistry, ServerSession, ServerState, ServerStatus,
    };

    async fn wait_for(
        session: &ServerSession,
        mut predicate: impl FnMut(&ServerState) -> bool,
        budget: Duration,
    ) -> ServerState {
        tokio::time::timeout(budget, async {
            let mut rx = session.watch();
            loop {
                {
                    let state = rx.borrow();
                    if predicate(&state) {
                        return state.clone();
                    }
                }
                rx.changed()
                    .await
                    .expect("session state channel closed unexpectedly");
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for expected session state"))
    }

    async fn running_session(language_id: &str, root: &Path) -> Arc<ServerSession> {
        let registry = ServerRegistry::new();
        let session = registry
            .get_or_start(language_id, root)
            .await
            .unwrap_or_else(|_| panic!("{language_id} server should be discoverable on PATH"));
        let state = wait_for(
            &session,
            |s| {
                matches!(
                    s.status,
                    ServerStatus::Running | ServerStatus::Failed { .. }
                )
            },
            Duration::from_secs(90),
        )
        .await;
        assert_eq!(
            state.status,
            ServerStatus::Running,
            "{language_id} server did not reach Running: {:?}",
            state.status
        );
        assert!(
            state.capabilities.is_some(),
            "{language_id} server completed no capability handshake"
        );
        session
    }

    /// Real servers (especially on a cold index) can answer `CONTENT_MODIFIED`
    /// or an empty symbol list for a moment after `didOpen` -- same retry
    /// shape every other Phase 7 real-server test in this crate uses.
    async fn symbols_until_nonempty(
        navigation: &Navigation,
        uri: &async_lsp::lsp_types::Url,
        budget: Duration,
    ) -> Vec<lux_lsp::SymbolEntry> {
        tokio::time::timeout(budget, async {
            loop {
                if let Ok(symbols) = navigation.document_symbols(uri).await
                    && !symbols.is_empty()
                {
                    return symbols;
                }
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!("timed out waiting for a real, non-empty documentSymbol response")
        })
    }

    async fn assert_server_works(language_id: &str, root: &Path, file: &str, text: &str) {
        let session = running_session(language_id, root).await;
        let sync = DocumentSync::new(session.clone());
        let navigation = Navigation::new(session.clone());
        let uri =
            async_lsp::lsp_types::Url::from_file_path(root.canonicalize().unwrap().join(file))
                .unwrap();

        sync.open(uri.clone(), language_id, text.to_string())
            .expect("didOpen should succeed");

        let symbols = symbols_until_nonempty(&navigation, &uri, Duration::from_secs(60)).await;
        assert!(
            !symbols.is_empty(),
            "{language_id}: expected at least one real document symbol"
        );

        session.stop().await;
    }

    #[tokio::test]
    async fn rust_analyzer_works() {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        let text = "pub fn add(a: i32, b: i32) -> i32 { a + b }\n";
        std::fs::write(dir.path().join("src/lib.rs"), text).unwrap();

        assert_server_works("rust", dir.path(), "src/lib.rs", text).await;
    }

    #[tokio::test]
    async fn typescript_language_server_works() {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(
            dir.path().join("tsconfig.json"),
            "{\"compilerOptions\":{\"target\":\"es2020\"}}\n",
        )
        .unwrap();
        let text = "export function add(a: number, b: number): number {\n  return a + b;\n}\n";
        std::fs::write(dir.path().join("fixture.ts"), text).unwrap();

        // typescript-language-server resolves `typescript` from a real
        // local node_modules, not a global install. Pinned to the `5.x`
        // line deliberately, not `latest`: TypeScript's new native
        // (Go-ported) `7.x` package has no `lib/tsserver.js` at all --
        // typescript-language-server can't use it and fails with
        // "Could not find a valid TypeScript installation", which reads
        // like a missing dependency rather than a version mismatch.
        // Found by installing `latest` first and getting exactly that
        // error against a package that was genuinely present.
        let status = std::process::Command::new("npm")
            .args(["install", "typescript@5", "--no-save", "--loglevel=error"])
            .current_dir(dir.path())
            .status()
            .expect("npm should be on PATH");
        assert!(status.success(), "npm install typescript@5 failed");

        assert_server_works("typescript", dir.path(), "fixture.ts", text).await;
    }

    #[tokio::test]
    async fn pyright_works() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let text = "def add(a: int, b: int) -> int:\n    return a + b\n";
        std::fs::write(dir.path().join("fixture.py"), text).unwrap();

        assert_server_works("python", dir.path(), "fixture.py", text).await;
    }

    #[tokio::test]
    async fn gopls_works() {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(dir.path().join("go.mod"), "module fixture\n\ngo 1.21\n").unwrap();
        let text = "package fixture\n\nfunc Add(a int, b int) int {\n\treturn a + b\n}\n";
        std::fs::write(dir.path().join("fixture.go"), text).unwrap();

        assert_server_works("go", dir.path(), "fixture.go", text).await;
    }

    #[tokio::test]
    async fn clangd_works() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let text = "int add(int a, int b) {\n    return a + b;\n}\n";
        std::fs::write(dir.path().join("fixture.c"), text).unwrap();

        assert_server_works("c", dir.path(), "fixture.c", text).await;
    }
}
