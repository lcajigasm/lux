//! Candidate collection and query search logic for Goto Anything.

use super::{GotoItem, GotoItemKind};
use lux_ui::goto_anything::{self, RecentFiles};

pub(super) fn collect_candidates(app: &crate::app::App) -> Vec<String> {
    let mut list = Vec::new();

    // 1. Open tabs
    for tab in app.documents.tabs.tabs() {
        if let Some(entry) = app.documents.entries.iter().find(|e| e.tab_id == tab.id) {
            if let Some(path) = &entry.path {
                let s = path.to_string_lossy().to_string();
                if !list.contains(&s) {
                    list.push(s);
                }
            } else if !list.contains(&tab.title) {
                list.push(tab.title.clone());
            }
        }
    }

    // 2. Current document path
    if let Some(path) = &app.documents.path {
        let s = path.to_string_lossy().to_string();
        if !list.contains(&s) {
            list.push(s);
        }
    }

    // 3. Workspace files if folder is opened in sidebar
    if let Some(sidebar) = &app.ui.sidebar
        && let Some(root) = &sidebar.workspace_root
        && let Ok(index) = lux_workspace::WorkspaceIndex::build(root)
    {
        for f in index.files() {
            let rel = f.strip_prefix(root).unwrap_or(f);
            let s = rel.to_string_lossy().to_string();
            if !list.contains(&s) {
                list.push(s);
            }
        }
    }

    list
}

pub(super) fn compute_items(
    app: &crate::app::App,
    mode: super::GotoMode,
    raw_query: &str,
    cached_candidates: &[String],
    recent_files: &RecentFiles,
) -> Vec<GotoItem> {
    if mode == super::GotoMode::ProjectSymbols {
        return compute_project_symbols(app, raw_query, cached_candidates);
    }

    let query = lux_ui::goto_anything::Query::parse(raw_query);

    // 1. Symbol mode: "@symbol" or "file@symbol"
    if let Some(sym_query) = &query.symbol {
        return compute_symbol_results(app, &query.text, sym_query, cached_candidates);
    }

    // 2. Term search mode: "#term" or "file#term"
    if let Some(term_query) = &query.term {
        return compute_term_results(app, &query.text, term_query, cached_candidates);
    }

    // 3. Line mode: ":line", ":line:col", "file:line", "file:line:col"
    if raw_query.starts_with(':') || query.line.is_some() {
        return compute_line_results(app, &query, cached_candidates, recent_files);
    }

    // 4. File search mode
    let candidate_strs = cached_candidates.iter().map(String::as_str);
    let matches = goto_anything::search_with_recency(raw_query, candidate_strs, recent_files);
    matches
        .into_iter()
        .map(|m| GotoItem {
            label: m.path.to_string(),
            detail: String::new(),
            path: m.path.to_string(),
            line: m.line,
            col: m.col,
            kind: GotoItemKind::File {
                line: m.line,
                col: m.col,
            },
        })
        .collect()
}

fn compute_project_symbols(
    app: &crate::app::App,
    raw_query: &str,
    cached_candidates: &[String],
) -> Vec<GotoItem> {
    let mut index = app.ui.session.symbol_index.clone();

    // 1. Index open tabs first so newly typed buffers are immediate
    for entry in &app.documents.entries {
        if let Some(path) = &entry.path {
            index.index_file_content(path, &entry.buffer.text(), None);
        }
    }
    if let Some(path) = &app.documents.path {
        index.index_file_content(path, &app.buffer.text(), None);
    }

    // 2. Index workspace candidates from disk
    if let Some(sidebar) = &app.ui.sidebar
        && let Some(root) = &sidebar.workspace_root
    {
        index.set_root(root);
        for rel in cached_candidates {
            let full = root.join(rel);
            index.index_file_path(&full);
        }
    } else {
        for cand in cached_candidates {
            let path = std::path::Path::new(cand);
            if path.exists() {
                index.index_file_path(path);
            }
        }
    }

    let results = index.search(raw_query);
    results
        .into_iter()
        .map(|s| {
            let path_str = s.path.to_string_lossy().to_string();
            GotoItem {
                label: s.name,
                detail: format!("{}:{}", path_str, s.line),
                path: path_str,
                line: Some(s.line),
                col: Some(s.column),
                kind: GotoItemKind::Symbol {
                    kind_badge: s.kind.badge(),
                    line: s.line,
                    col: s.column,
                },
            }
        })
        .collect()
}

fn compute_symbol_results(
    app: &crate::app::App,
    file_filter: &str,
    sym_query: &str,
    cached_candidates: &[String],
) -> Vec<GotoItem> {
    let (target_path, symbols) = if file_filter.is_empty() {
        let path = app
            .documents
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "current".to_string());
        let lang = app
            .syntax
            .as_ref()
            .map(|s| s.language)
            .or_else(|| app.documents.language());
        let syms = if let Some(lang) = lang {
            let mut doc = lux_syntax::Document::new(lang);
            let text = app.buffer.text();
            doc.set_text(&text);
            lux_syntax::symbols::document_symbols(&doc, &text)
        } else {
            lux_syntax::symbols::extract_symbols_from_source(&app.buffer.text(), Some(&path))
        };
        (path, syms)
    } else {
        let best_file = best_file_candidate(file_filter, cached_candidates);
        let path = best_file.unwrap_or_else(|| file_filter.to_string());
        let syms = if let Some(entry) = app
            .documents
            .entries
            .iter()
            .find(|e| e.path.as_ref().is_some_and(|p| p.to_string_lossy() == path))
        {
            lux_syntax::symbols::extract_symbols_from_source(&entry.buffer.text(), Some(&path))
        } else if let Ok(content) = std::fs::read_to_string(&path) {
            lux_syntax::symbols::extract_symbols_from_source(&content, Some(&path))
        } else {
            Vec::new()
        };
        (path, syms)
    };

    let sym_query_lower = sym_query.to_lowercase();
    symbols
        .into_iter()
        .filter(|s| {
            if sym_query.is_empty() {
                true
            } else {
                s.name.to_lowercase().contains(&sym_query_lower)
            }
        })
        .map(|s| GotoItem {
            label: s.name,
            detail: format!("{} :{}", s.kind.badge(), s.line),
            path: target_path.clone(),
            line: Some(s.line),
            col: Some(s.column),
            kind: GotoItemKind::Symbol {
                kind_badge: s.kind.badge(),
                line: s.line,
                col: s.column,
            },
        })
        .collect()
}

fn compute_term_results(
    app: &crate::app::App,
    file_filter: &str,
    term_query: &str,
    cached_candidates: &[String],
) -> Vec<GotoItem> {
    let (target_path, content) = if file_filter.is_empty() {
        let path = app
            .documents
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "current".to_string());
        (path, app.buffer.text())
    } else {
        let best_file = best_file_candidate(file_filter, cached_candidates);
        let path = best_file.unwrap_or_else(|| file_filter.to_string());
        let text = if let Some(entry) = app
            .documents
            .entries
            .iter()
            .find(|e| e.path.as_ref().is_some_and(|p| p.to_string_lossy() == path))
        {
            entry.buffer.text()
        } else {
            std::fs::read_to_string(&path).unwrap_or_default()
        };
        (path, text)
    };

    let term_lower = term_query.to_lowercase();
    let mut items = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let line_num = (idx + 1) as u32;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if term_query.is_empty() || trimmed.to_lowercase().contains(&term_lower) {
            items.push(GotoItem {
                label: trimmed.to_string(),
                detail: format!(":{}", line_num),
                path: target_path.clone(),
                line: Some(line_num),
                col: Some(1),
                kind: GotoItemKind::Term {
                    line: line_num,
                    col: 1,
                    preview: trimmed.to_string(),
                },
            });
            if items.len() >= 60 {
                break;
            }
        }
    }
    items
}

fn compute_line_results(
    app: &crate::app::App,
    query: &lux_ui::goto_anything::Query,
    cached_candidates: &[String],
    recent_files: &RecentFiles,
) -> Vec<GotoItem> {
    if query.text.is_empty() {
        let path = app
            .documents
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "Current document".to_string());
        let detail = match (query.line, query.col) {
            (Some(l), Some(c)) => format!(":line {l}:{c}"),
            (Some(l), None) => format!(":line {l}"),
            _ => ":line".to_string(),
        };
        vec![GotoItem {
            label: path.clone(),
            detail,
            path,
            line: query.line,
            col: query.col,
            kind: GotoItemKind::File {
                line: query.line,
                col: query.col,
            },
        }]
    } else {
        let candidate_strs = cached_candidates.iter().map(String::as_str);
        let matches = goto_anything::search_with_recency(&query.text, candidate_strs, recent_files);
        let detail = match (query.line, query.col) {
            (Some(l), Some(c)) => format!(":line {l}:{c}"),
            (Some(l), None) => format!(":line {l}"),
            _ => ":line".to_string(),
        };
        matches
            .into_iter()
            .map(|m| GotoItem {
                label: m.path.to_string(),
                detail: detail.clone(),
                path: m.path.to_string(),
                line: query.line,
                col: query.col,
                kind: GotoItemKind::File {
                    line: query.line,
                    col: query.col,
                },
            })
            .collect()
    }
}

fn best_file_candidate(file_filter: &str, cached_candidates: &[String]) -> Option<String> {
    let candidate_strs = cached_candidates.iter().map(String::as_str);
    let matches = goto_anything::search(file_filter, candidate_strs);
    matches.first().map(|m| m.path.to_string())
}
