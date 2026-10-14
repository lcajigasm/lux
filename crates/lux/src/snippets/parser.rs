//! Parsers for Sublime Text snippets (`.sublime-snippet` XML format) and JSON snippets.

use serde::Deserialize;
use std::collections::HashMap;

/// A parsed snippet definition ready for registry indexing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSnippet {
    pub name: String,
    pub trigger: String,
    pub content: String,
    pub scope: Option<String>,
    pub description: Option<String>,
}

/// Parses a Sublime Text `.sublime-snippet` XML format file.
pub fn parse_sublime_snippet(input: &str, default_name: &str) -> Option<RawSnippet> {
    let content = extract_cdata_or_text(input, "content")?;
    let trigger = extract_tag_text(input, "tabTrigger").unwrap_or_default();
    let scope = extract_tag_text(input, "scope");
    let description = extract_tag_text(input, "description");

    if trigger.is_empty() && description.is_none() {
        return None;
    }

    Some(RawSnippet {
        name: description
            .clone()
            .unwrap_or_else(|| default_name.to_string()),
        trigger,
        content,
        scope,
        description,
    })
}

fn extract_tag_text(xml: &str, tag: &str) -> Option<String> {
    let open_tag = format!("<{tag}>");
    let close_tag = format!("</{tag}>");
    let start_idx = xml.find(&open_tag)? + open_tag.len();
    let end_idx = xml[start_idx..].find(&close_tag)? + start_idx;
    let text = xml[start_idx..end_idx].trim();
    if text.is_empty() {
        None
    } else {
        Some(unescape_xml(text))
    }
}

fn extract_cdata_or_text(xml: &str, tag: &str) -> Option<String> {
    let open_tag = format!("<{tag}>");
    let close_tag = format!("</{tag}>");
    let start_idx = xml.find(&open_tag)? + open_tag.len();
    let end_idx = xml[start_idx..].find(&close_tag)? + start_idx;
    let section = &xml[start_idx..end_idx];

    let cdata_start = "<![CDATA[";
    let cdata_end = "]]>";
    if let (Some(cs), Some(ce)) = (section.find(cdata_start), section.rfind(cdata_end))
        && cs < ce
    {
        let inner = &section[cs + cdata_start.len()..ce];
        let raw = inner.strip_prefix('\n').unwrap_or(inner);
        let raw = raw.strip_suffix('\n').unwrap_or(raw);
        return Some(raw.to_string());
    }

    let trimmed = section.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(unescape_xml(trimmed))
    }
}

fn unescape_xml(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SnippetBody {
    Single(String),
    Multi(Vec<String>),
}

#[derive(Deserialize)]
struct JsonSnippetEntry {
    #[serde(alias = "tabTrigger", alias = "prefix")]
    trigger: Option<String>,
    #[serde(alias = "content", alias = "body")]
    body: Option<SnippetBody>,
    scope: Option<String>,
    description: Option<String>,
}

/// Parses snippets from a JSON string (either array of snippets or dictionary of named snippets).
pub fn parse_json_snippets(json_str: &str, file_name: &str) -> Vec<RawSnippet> {
    let mut results = Vec::new();

    if let Ok(entries) = serde_json::from_str::<Vec<JsonSnippetEntry>>(json_str) {
        for (idx, entry) in entries.into_iter().enumerate() {
            if let Some(raw) = entry_to_raw_snippet(entry, &format!("{file_name} #{idx}")) {
                results.push(raw);
            }
        }
        return results;
    }

    if let Ok(map) = serde_json::from_str::<HashMap<String, JsonSnippetEntry>>(json_str) {
        for (name, entry) in map {
            if let Some(raw) = entry_to_raw_snippet(entry, &name) {
                results.push(raw);
            }
        }
        return results;
    }

    if let Ok(single) = serde_json::from_str::<JsonSnippetEntry>(json_str)
        && let Some(raw) = entry_to_raw_snippet(single, file_name)
    {
        results.push(raw);
    }

    results
}

fn entry_to_raw_snippet(entry: JsonSnippetEntry, default_name: &str) -> Option<RawSnippet> {
    let content = match entry.body? {
        SnippetBody::Single(s) => s,
        SnippetBody::Multi(lines) => lines.join("\n"),
    };
    let trigger = entry.trigger.unwrap_or_default();
    let name = entry
        .description
        .clone()
        .unwrap_or_else(|| default_name.to_string());

    Some(RawSnippet {
        name,
        trigger,
        content,
        scope: entry.scope,
        description: entry.description,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sublime_snippet_with_cdata() {
        let xml = r#"<snippet>
    <content><![CDATA[
fn ${1:name}($2) {
	$0
}
]]></content>
    <tabTrigger>fn</tabTrigger>
    <scope>source.rust</scope>
    <description>Function definition</description>
</snippet>"#;

        let snippet = parse_sublime_snippet(xml, "fn").unwrap();
        assert_eq!(snippet.trigger, "fn");
        assert_eq!(snippet.content, "fn ${1:name}($2) {\n\t$0\n}");
        assert_eq!(snippet.scope.as_deref(), Some("source.rust"));
        assert_eq!(snippet.description.as_deref(), Some("Function definition"));
    }

    #[test]
    fn parse_json_dictionary_snippets() {
        let json = r#"{
            "Print to console": {
                "prefix": "clg",
                "body": [
                    "console.log($1);",
                    "$0"
                ],
                "description": "Log output to console",
                "scope": "source.js, source.ts"
            }
        }"#;

        let snippets = parse_json_snippets(json, "test.json");
        assert_eq!(snippets.len(), 1);
        let s = &snippets[0];
        assert_eq!(s.trigger, "clg");
        assert_eq!(s.content, "console.log($1);\n$0");
        assert_eq!(s.scope.as_deref(), Some("source.js, source.ts"));
        assert_eq!(s.description.as_deref(), Some("Log output to console"));
    }

    #[test]
    fn parse_json_array_snippets() {
        let json = r#"[
            {
                "tabTrigger": "lorem",
                "content": "Lorem ipsum dolor sit amet",
                "description": "Lorem ipsum text"
            }
        ]"#;

        let snippets = parse_json_snippets(json, "lorem.json");
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].trigger, "lorem");
        assert_eq!(snippets[0].content, "Lorem ipsum dolor sit amet");
        assert!(snippets[0].scope.is_none());
    }
}
