//! Sublime scope selectors against a buffer's base scope (`source.rust`,
//! `text.html.markdown`). A selector element matches a scope that equals
//! it or extends it by dotted parts; `,` and `|` separate alternatives;
//! `a - b` matches `a` but not `b`. Lux only knows a buffer's base scope,
//! so descendant selectors (`text.html source.js`) match on their last
//! element only when it is the base scope itself, never inside.

/// The best score among the selector's alternatives, or `None` when none
/// matches. A longer (more specific) matching element scores higher, so
/// `source.c++` beats `source` for a C++ buffer.
pub fn score(selector: &str, scope: &str) -> Option<u32> {
    selector
        .split([',', '|'])
        .filter_map(|alternative| score_alternative(alternative, scope))
        .max()
}

fn score_alternative(alternative: &str, scope: &str) -> Option<u32> {
    let mut parts = alternative.split(" - ");
    let include = parts.next()?.trim();
    if include.is_empty() {
        return None;
    }
    // Descendant selectors: the base scope is the outermost and only
    // scope, so only a one-element path can match.
    if include.split_whitespace().count() != 1 {
        return None;
    }
    let found = element_score(include, scope)?;
    for exclude in parts {
        let exclude = exclude.trim();
        if !exclude.is_empty() && element_score(exclude, scope).is_some() {
            return None;
        }
    }
    Some(found)
}

fn element_score(element: &str, scope: &str) -> Option<u32> {
    let element = element.trim_matches(|c| c == '(' || c == ')').trim();
    let matches = scope == element
        || scope
            .strip_prefix(element)
            .is_some_and(|rest| rest.starts_with('.'));
    matches.then(|| element.split('.').count() as u32)
}
