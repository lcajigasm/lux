//! Imports IntelliJ/JetBrains code-style XML (e.g. the `<indentOptions>`
//! block inside `.idea/codeStyles/Project.xml`): scans for every
//! `<option name="..." value="...">` element in the document, wherever it
//! appears, and maps recognized names onto Lux's settings namespace.
//!
//! A full DOM-aware reader that only looked inside `<indentOptions>` would
//! be more precise about *where* an option came from, but this
//! deliverable's own wording only asks that imports honestly report what
//! was imported and what was ignored — a flat scan already satisfies that:
//! every `<option>` found is either imported or explicitly ignored, none
//! silently dropped because of where in the tree it sits. Parsing is via
//! `quick-xml`, a real streaming XML reader — hand-rolling attribute
//! parsing for even this narrow a slice of XML (quoting, escaping, both
//! self-closing and open/close tag forms) would duplicate real, tricky,
//! already-solved work for no reason.
//!
//! Source keys `TAB_SIZE`, `INDENT_SIZE`, and `USE_TAB_CHARACTER` are
//! confirmed directly against JetBrains' own `intellij-community` source
//! (`CommonCodeStyleSettings.java`'s `IndentOptions` class) — the
//! strongest evidence reachable, since that source *is* what produces
//! this XML. See
//! `docs/receipts/phase-05-importadores-ajustes-externos.md`.

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde_json::json;

use super::{ImportReport, Mapped, push};

pub fn import(xml: &str) -> Result<ImportReport, quick_xml::Error> {
    let mut report = ImportReport::default();
    let mut reader = Reader::from_str(xml);

    loop {
        match reader.read_event()? {
            Event::Eof => break,
            Event::Start(tag) | Event::Empty(tag) if tag.name().as_ref() == b"option" => {
                let mut name = None;
                let mut value = None;
                for attr in tag.attributes().filter_map(Result::ok) {
                    // The `name`/`value` attributes this module reads are
                    // always plain identifiers, numbers, or "true"/"false"
                    // — never XML entities — so a lossy UTF-8 decode of
                    // the raw bytes is enough; no need for full XML-entity
                    // unescaping here.
                    let raw = String::from_utf8_lossy(&attr.value).into_owned();
                    match attr.key.as_ref() {
                        b"name" => name = Some(raw),
                        b"value" => value = Some(raw),
                        _ => {}
                    }
                }
                if let (Some(name), Some(value)) = (name, value) {
                    push(&mut report, name.clone(), map_key(&name, &value));
                }
            }
            _ => {}
        }
    }

    Ok(report)
}

fn map_key(name: &str, value: &str) -> Mapped {
    match name {
        "TAB_SIZE" => value
            .parse::<i64>()
            .map_or(Mapped::Unsupported("integer"), |n| {
                Mapped::Imported("editor.tabWidth", json!(n))
            }),
        "INDENT_SIZE" => value
            .parse::<i64>()
            .map_or(Mapped::Unsupported("integer"), |n| {
                Mapped::Imported("editor.indentSize", json!(n))
            }),
        "USE_TAB_CHARACTER" => match value {
            "true" => Mapped::Imported("editor.indentStyle", json!("tab")),
            "false" => Mapped::Imported("editor.indentStyle", json!("space")),
            _ => Mapped::Unsupported("boolean"),
        },
        _ => Mapped::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECT_XML: &str = r#"<code_scheme name="Project" version="173">
  <option name="LINE_SEPARATOR" value="\n" />
  <indentOptions>
    <option name="INDENT_SIZE" value="4" />
    <option name="TAB_SIZE" value="4" />
    <option name="USE_TAB_CHARACTER" value="false" />
  </indentOptions>
</code_scheme>"#;

    #[test]
    fn imports_known_options_from_a_real_shaped_code_style_xml() {
        let report = import(PROJECT_XML).unwrap();
        assert!(report.imported.iter().any(|s| s.source_key == "INDENT_SIZE"
            && s.lux_key == "editor.indentSize"
            && s.value == json!(4)));
        assert!(report.imported.iter().any(|s| s.source_key == "TAB_SIZE"
            && s.lux_key == "editor.tabWidth"
            && s.value == json!(4)));
        assert!(
            report
                .imported
                .iter()
                .any(|s| s.source_key == "USE_TAB_CHARACTER"
                    && s.lux_key == "editor.indentStyle"
                    && s.value == json!("space"))
        );
    }

    #[test]
    fn use_tab_character_true_maps_to_tab_indent_style() {
        let report = import(r#"<r><option name="USE_TAB_CHARACTER" value="true"/></r>"#).unwrap();
        assert_eq!(report.imported[0].value, json!("tab"));
    }

    #[test]
    fn an_unrecognized_option_name_is_reported_as_unknown() {
        let report = import(r#"<r><option name="LINE_SEPARATOR" value="\n"/></r>"#).unwrap();
        assert_eq!(report.imported.len(), 0);
        assert_eq!(report.ignored[0].source_key, "LINE_SEPARATOR");
        assert_eq!(
            report.ignored[0].reason,
            super::super::IgnoreReason::UnknownKey
        );
    }

    #[test]
    fn a_recognized_option_with_an_unparseable_value_is_unsupported() {
        let report = import(r#"<r><option name="TAB_SIZE" value="four"/></r>"#).unwrap();
        assert_eq!(report.imported.len(), 0);
        assert_eq!(
            report.ignored[0].reason,
            super::super::IgnoreReason::UnsupportedValue {
                expected: "integer"
            }
        );
    }

    #[test]
    fn options_nested_arbitrarily_deep_are_still_found() {
        let xml = r#"<a><b><c><option name="TAB_SIZE" value="2"/></c></b></a>"#;
        let report = import(xml).unwrap();
        assert_eq!(report.imported.len(), 1);
    }

    #[test]
    fn non_option_elements_are_not_reported_at_all() {
        let report = import(PROJECT_XML).unwrap();
        // 4 <option> elements total in PROJECT_XML: LINE_SEPARATOR (unknown)
        // + the three indentOptions ones (all imported) — nothing extra
        // from <code_scheme>/<indentOptions> themselves.
        assert_eq!(report.imported.len() + report.ignored.len(), 4);
    }

    #[test]
    fn malformed_xml_is_a_real_error() {
        assert!(import("<r><option name=\"TAB_SIZE\" value=\"4\"").is_err());
    }
}
