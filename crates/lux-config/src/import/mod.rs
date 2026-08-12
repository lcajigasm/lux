//! Schema-aware settings importers (Phase 5 deliverable 4). Sublime Text,
//! VS Code, and JetBrains each have their own settings vocabulary and file
//! format; each importer here maps a modest, verified-real subset of
//! source keys onto Lux's own settings namespace (`editor.tabWidth`,
//! `editor.indentStyle`, ... — the same dotted-key shape `layering`'s
//! JSON-object settings already use) and reports, honestly, what it
//! recognized and imported versus what it explicitly ignored and why —
//! "conscientes del esquema... que informen honestamente de qué se
//! importó y qué se ignoró", per this deliverable's own wording. A key
//! this crate doesn't have an equivalent for, and a key it recognizes but
//! whose value doesn't parse as the type Lux expects, are two different,
//! separately-reported outcomes — not the same "ignored" bucket.
//!
//! Every mapped source key was either confirmed against the vendor's own
//! documentation or source code, or — where that wasn't reachable — noted
//! as coming from well-established prior knowledge instead of presented as
//! uniformly verified. See
//! `docs/receipts/phase-05-importadores-ajustes-externos.md` for exactly
//! which is which.

pub mod jetbrains;
pub mod sublime;
pub mod vscode;

use serde_json::Value;

/// Why a source setting wasn't imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IgnoreReason {
    /// Lux has no equivalent setting for this source key.
    UnknownKey,
    /// The key is recognized, but its value wasn't the type Lux expects
    /// for it (e.g. a string where an integer was expected).
    UnsupportedValue { expected: &'static str },
}

/// One source setting that was successfully mapped onto a Lux setting.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedSetting {
    pub source_key: String,
    pub lux_key: &'static str,
    pub value: Value,
}

/// One source setting that was recognized but explicitly not imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoredSetting {
    pub source_key: String,
    pub reason: IgnoreReason,
}

/// What one import pass produced: every setting Lux recognized and
/// imported, and every one it explicitly ignored, with why. Every source
/// key that was present ends up in exactly one of the two lists — nothing
/// is silently dropped.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportReport {
    pub imported: Vec<ImportedSetting>,
    pub ignored: Vec<IgnoredSetting>,
}

/// The outcome of trying to map one source key/value pair onto Lux's
/// settings namespace — what each source module's `map_key` returns.
pub(crate) enum Mapped {
    Imported(&'static str, Value),
    Unsupported(&'static str),
    Unknown,
}

/// Shared driver for the two JSON-based importers (`sublime`, `vscode`):
/// walks a parsed settings object's top-level keys, applies `map_key` to
/// each, and sorts the result into `ImportReport`'s two lists. Not used by
/// `jetbrains`, whose source format is XML, not a JSON object.
pub(crate) fn report_from_object(
    value: Value,
    map_key: impl Fn(&str, &Value) -> Mapped,
) -> ImportReport {
    let mut report = ImportReport::default();
    if let Value::Object(entries) = value {
        for (key, value) in entries {
            push(&mut report, key.clone(), map_key(&key, &value));
        }
    }
    report
}

pub(crate) fn push(report: &mut ImportReport, source_key: String, mapped: Mapped) {
    match mapped {
        Mapped::Imported(lux_key, value) => report.imported.push(ImportedSetting {
            source_key,
            lux_key,
            value,
        }),
        Mapped::Unsupported(expected) => report.ignored.push(IgnoredSetting {
            source_key,
            reason: IgnoreReason::UnsupportedValue { expected },
        }),
        Mapped::Unknown => report.ignored.push(IgnoredSetting {
            source_key,
            reason: IgnoreReason::UnknownKey,
        }),
    }
}
