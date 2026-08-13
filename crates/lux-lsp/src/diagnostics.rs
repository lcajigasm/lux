//! `publishDiagnostics` state as a filterable, navigable problems view.
//! Reception and continuous updates live in `session.rs` (the `Router`
//! handler that feeds `ServerSession`'s diagnostics channel); this module
//! is the query layer on top of it.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Url};
use tokio::sync::watch;

use crate::session::ServerSession;

/// Diagnostic counts by severity across the whole workspace -- what a
/// problems panel's header or a status-bar summary needs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiagnosticsSummary {
    pub errors: usize,
    pub warnings: usize,
    pub information: usize,
    pub hints: usize,
}

/// A filterable, navigable view over one session's current diagnostics.
pub struct Diagnostics {
    session: Arc<ServerSession>,
}

impl Diagnostics {
    pub fn new(session: Arc<ServerSession>) -> Self {
        Self { session }
    }

    /// Every current diagnostic, across all documents, in a stable
    /// (document, then position) order.
    pub fn all(&self) -> Vec<(Url, Diagnostic)> {
        flattened_sorted(&self.session.diagnostics())
    }

    /// Diagnostics for one document, in the order the server reported.
    pub fn for_document(&self, uri: &Url) -> Vec<Diagnostic> {
        self.session
            .diagnostics()
            .get(uri)
            .cloned()
            .unwrap_or_default()
    }

    /// Only diagnostics at least as severe as `min_severity` (numerically
    /// `<=`, since [`DiagnosticSeverity::ERROR`] is `1` and
    /// [`DiagnosticSeverity::HINT`] is `4`) -- the "problems panel filtered
    /// to errors and warnings" use case.
    pub fn filtered(&self, min_severity: DiagnosticSeverity) -> Vec<(Url, Diagnostic)> {
        filter_by_severity(&self.session.diagnostics(), min_severity)
    }

    pub fn summary(&self) -> DiagnosticsSummary {
        summarize(&self.session.diagnostics())
    }

    /// The next diagnostic strictly after `(uri, position)`, in the stable
    /// order `all()` uses, wrapping around to the first one. `None` if
    /// there are no diagnostics at all.
    pub fn next_after(&self, uri: &Url, position: Position) -> Option<(Url, Diagnostic)> {
        navigate(&self.session.diagnostics(), uri, position, Direction::Next)
    }

    /// Symmetric to [`Self::next_after`], wrapping to the last diagnostic.
    pub fn previous_before(&self, uri: &Url, position: Position) -> Option<(Url, Diagnostic)> {
        navigate(
            &self.session.diagnostics(),
            uri,
            position,
            Direction::Previous,
        )
    }

    /// Subscribe to diagnostics updates -- event-driven, no polling.
    pub fn watch(&self) -> watch::Receiver<Arc<BTreeMap<Url, Vec<Diagnostic>>>> {
        self.session.watch_diagnostics()
    }
}

/// The server may omit `severity`; per the spec that's up to the client to
/// interpret, and the conservative choice for a filterable panel is to
/// never let an unmarked diagnostic get hidden by a severity filter.
fn effective_severity(diagnostic: &Diagnostic) -> DiagnosticSeverity {
    diagnostic.severity.unwrap_or(DiagnosticSeverity::ERROR)
}

fn flattened_sorted(map: &BTreeMap<Url, Vec<Diagnostic>>) -> Vec<(Url, Diagnostic)> {
    let mut all: Vec<(Url, Diagnostic)> = map
        .iter()
        .flat_map(|(uri, diags)| diags.iter().map(move |d| (uri.clone(), d.clone())))
        .collect();
    all.sort_by(|(a_uri, a), (b_uri, b)| sort_key(a_uri, a).cmp(&sort_key(b_uri, b)));
    all
}

fn sort_key<'a>(uri: &'a Url, diagnostic: &Diagnostic) -> (&'a Url, u32, u32) {
    (
        uri,
        diagnostic.range.start.line,
        diagnostic.range.start.character,
    )
}

fn filter_by_severity(
    map: &BTreeMap<Url, Vec<Diagnostic>>,
    min_severity: DiagnosticSeverity,
) -> Vec<(Url, Diagnostic)> {
    flattened_sorted(map)
        .into_iter()
        .filter(|(_, d)| effective_severity(d) <= min_severity)
        .collect()
}

fn summarize(map: &BTreeMap<Url, Vec<Diagnostic>>) -> DiagnosticsSummary {
    let mut summary = DiagnosticsSummary::default();
    for diagnostic in map.values().flatten() {
        match effective_severity(diagnostic) {
            DiagnosticSeverity::ERROR => summary.errors += 1,
            DiagnosticSeverity::WARNING => summary.warnings += 1,
            DiagnosticSeverity::INFORMATION => summary.information += 1,
            DiagnosticSeverity::HINT => summary.hints += 1,
            _ => {}
        }
    }
    summary
}

enum Direction {
    Next,
    Previous,
}

fn navigate(
    map: &BTreeMap<Url, Vec<Diagnostic>>,
    uri: &Url,
    position: Position,
    direction: Direction,
) -> Option<(Url, Diagnostic)> {
    let all = flattened_sorted(map);
    if all.is_empty() {
        return None;
    }
    let cursor = (uri, position.line, position.character);
    let found = match direction {
        Direction::Next => all.iter().find(|(u, d)| sort_key(u, d) > cursor),
        Direction::Previous => all.iter().rev().find(|(u, d)| sort_key(u, d) < cursor),
    };
    found.cloned().or_else(|| match direction {
        Direction::Next => all.first().cloned(),
        Direction::Previous => all.last().cloned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostic(
        line: u32,
        character: u32,
        severity: DiagnosticSeverity,
        message: &str,
    ) -> Diagnostic {
        Diagnostic {
            range: async_lsp::lsp_types::Range {
                start: Position { line, character },
                end: Position { line, character },
            },
            severity: Some(severity),
            message: message.into(),
            ..Default::default()
        }
    }

    fn uri(name: &str) -> Url {
        Url::parse(&format!("file:///{name}")).unwrap()
    }

    fn sample_map() -> BTreeMap<Url, Vec<Diagnostic>> {
        let mut map = BTreeMap::new();
        map.insert(
            uri("a.rs"),
            vec![
                diagnostic(5, 0, DiagnosticSeverity::WARNING, "a-warning"),
                diagnostic(1, 0, DiagnosticSeverity::ERROR, "a-error"),
            ],
        );
        map.insert(
            uri("b.rs"),
            vec![diagnostic(0, 0, DiagnosticSeverity::HINT, "b-hint")],
        );
        map
    }

    #[test]
    fn flattens_in_stable_document_then_position_order() {
        let ordered = flattened_sorted(&sample_map());
        let messages: Vec<&str> = ordered.iter().map(|(_, d)| d.message.as_str()).collect();
        assert_eq!(messages, vec!["a-error", "a-warning", "b-hint"]);
    }

    #[test]
    fn filters_by_minimum_severity() {
        let errors_only = filter_by_severity(&sample_map(), DiagnosticSeverity::ERROR);
        assert_eq!(errors_only.len(), 1);
        assert_eq!(errors_only[0].1.message, "a-error");

        let warnings_and_up = filter_by_severity(&sample_map(), DiagnosticSeverity::WARNING);
        let messages: Vec<&str> = warnings_and_up
            .iter()
            .map(|(_, d)| d.message.as_str())
            .collect();
        assert_eq!(messages, vec!["a-error", "a-warning"]);
    }

    #[test]
    fn unmarked_severity_is_never_hidden_by_a_filter() {
        let mut map = BTreeMap::new();
        let mut unmarked = diagnostic(0, 0, DiagnosticSeverity::HINT, "unmarked");
        unmarked.severity = None;
        map.insert(uri("a.rs"), vec![unmarked]);
        assert_eq!(filter_by_severity(&map, DiagnosticSeverity::ERROR).len(), 1);
    }

    #[test]
    fn summarizes_counts_per_severity() {
        let summary = summarize(&sample_map());
        assert_eq!(
            summary,
            DiagnosticsSummary {
                errors: 1,
                warnings: 1,
                information: 0,
                hints: 1
            }
        );
    }

    #[test]
    fn next_after_wraps_around_to_the_first() {
        let map = sample_map();
        let a = uri("a.rs");
        let b = uri("b.rs");

        let next = navigate(
            &map,
            &a,
            Position {
                line: 1,
                character: 0,
            },
            Direction::Next,
        )
        .unwrap();
        assert_eq!(next.1.message, "a-warning");

        // Past the very last diagnostic (b.rs's hint) -- wraps to the first.
        let wrapped = navigate(
            &map,
            &b,
            Position {
                line: 99,
                character: 0,
            },
            Direction::Next,
        )
        .unwrap();
        assert_eq!(wrapped.1.message, "a-error");
    }

    #[test]
    fn previous_before_wraps_around_to_the_last() {
        let map = sample_map();
        let a = uri("a.rs");

        let previous = navigate(
            &map,
            &a,
            Position {
                line: 5,
                character: 0,
            },
            Direction::Previous,
        )
        .unwrap();
        assert_eq!(previous.1.message, "a-error");

        // Before the very first diagnostic -- wraps to the last.
        let wrapped = navigate(
            &map,
            &a,
            Position {
                line: 0,
                character: 0,
            },
            Direction::Previous,
        )
        .unwrap();
        assert_eq!(wrapped.1.message, "b-hint");
    }

    #[test]
    fn navigation_over_no_diagnostics_returns_none() {
        let empty = BTreeMap::new();
        let u = uri("a.rs");
        assert!(navigate(&empty, &u, Position::default(), Direction::Next).is_none());
        assert!(navigate(&empty, &u, Position::default(), Direction::Previous).is_none());
    }
}

#[cfg(test)]
mod real_server;
