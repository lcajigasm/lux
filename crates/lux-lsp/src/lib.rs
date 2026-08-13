//! A real Language Server Protocol client. Phase 7's first deliverable
//! covers server management only: discovery, launch, the capability
//! handshake, clean shutdown, crash-triggered restart, one server per
//! language and project root, and real per-server status with `$/progress`.
//!
//! Later Phase 7 deliverables (document sync, diagnostics, completion,
//! navigation, code actions, rename, formatting) build on top of the
//! [`ServerSession`] this module manages, via the `async_lsp::lsp_types`
//! and `LanguageServer`/`LanguageClient` surface `async-lsp` re-exports.

mod code_actions;
mod completion;
mod diagnostics;
mod discovery;
mod formatting;
mod framing;
mod hover;
mod inlay_hints;
mod navigation;
mod registry;
mod rename;
mod semantic_tokens;
mod session;
mod signature;
mod sync;

pub use code_actions::{
    ApplyError, CodeActions, CodeActionsError, FileChange, preview as preview_workspace_edit,
};
pub use completion::{Completion, CompletionError};
pub use diagnostics::{Diagnostics, DiagnosticsSummary};
pub use discovery::{KNOWN_SERVERS, ServerSpec, spec_for_language};
pub use formatting::{
    FormatAndSaveError, Formatting, FormattingConfig, FormattingError, LanguageFormattingConfig,
};
pub use framing::{MAX_MESSAGE_BYTES, guard_stream};
pub use hover::{Hover, HoverError, HoverText};
pub use inlay_hints::{InlayHintEntry, InlayHints, InlayHintsError};
pub use navigation::{Navigation, NavigationError, SymbolEntry, WorkspaceSymbolEntry};
pub use registry::{LspError, ServerRegistry};
pub use rename::{
    Rename, RenameApplyError, RenameError, RenameHunk, RenamePreview, preview as preview_rename,
};
pub use semantic_tokens::{ResolvedToken, SemanticTokensClient, SemanticTokensError};
pub use session::{ProgressInfo, ServerSession, ServerState, ServerStatus};
pub use signature::{Signature, SignatureError, active_parameter_label, active_signature};
pub use sync::{DocumentSync, SyncError, TextEdit};
