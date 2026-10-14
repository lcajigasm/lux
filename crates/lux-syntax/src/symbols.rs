//! Syntax-derived symbol outline and extraction via Tree-sitter (ST07/ST08).

use crate::Document;

/// Symbol kind classification for outlines and Goto Symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Class,
    Trait,
    Interface,
    Enum,
    Type,
    Constant,
    Module,
    Heading,
    Other,
}

impl SymbolKind {
    /// Short badge string suitable for UI lists (e.g. "fn", "struct").
    pub fn badge(&self) -> &'static str {
        match self {
            Self::Function => "fn",
            Self::Method => "method",
            Self::Struct => "struct",
            Self::Class => "class",
            Self::Trait => "trait",
            Self::Interface => "interface",
            Self::Enum => "enum",
            Self::Type => "type",
            Self::Constant => "const",
            Self::Module => "mod",
            Self::Heading => "heading",
            Self::Other => "symbol",
        }
    }
}

/// One symbol extracted from a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub line: u32,
    pub column: u32,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// Extracts all top-level and nested symbols in `doc`'s parse tree.
#[must_use]
pub fn document_symbols(doc: &Document, source: &str) -> Vec<DocumentSymbol> {
    let Some(tree) = doc.tree() else {
        return extract_fallback_symbols(source);
    };

    let mut symbols = Vec::new();
    let mut cursor = tree.root_node().walk();

    loop {
        let node = cursor.node();
        if let Some(kind) = classify_node(node.kind())
            && let Some(name) = extract_node_name(&node, source)
        {
            let start_pos = node.start_position();
            symbols.push(DocumentSymbol {
                name,
                kind,
                line: (start_pos.row + 1) as u32,
                column: (start_pos.column + 1) as u32,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
            });
        }

        if cursor.goto_first_child() {
            continue;
        }

        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                if symbols.is_empty() {
                    return extract_fallback_symbols(source);
                }
                symbols.sort_by_key(|s| (s.line, s.column));
                return symbols;
            }
        }
    }
}

fn classify_node(kind: &str) -> Option<SymbolKind> {
    match kind {
        "function_item" | "function_definition" | "function_declaration" | "arrow_function" => {
            Some(SymbolKind::Function)
        }
        "method_definition" | "method_declaration" => Some(SymbolKind::Method),
        "struct_item" | "struct_specifier" => Some(SymbolKind::Struct),
        "class_declaration" | "class_definition" | "class_specifier" => Some(SymbolKind::Class),
        "trait_item" => Some(SymbolKind::Trait),
        "interface_declaration" => Some(SymbolKind::Interface),
        "enum_item" | "enum_declaration" | "enum_specifier" => Some(SymbolKind::Enum),
        "type_item" | "type_declaration" | "type_alias_declaration" | "type_spec" => {
            Some(SymbolKind::Type)
        }
        "const_item" | "static_item" => Some(SymbolKind::Constant),
        "mod_item" | "namespace_definition" => Some(SymbolKind::Module),
        "impl_item" => Some(SymbolKind::Struct),
        "atx_heading" | "setext_heading" => Some(SymbolKind::Heading),
        _ => None,
    }
}

fn extract_node_name(node: &tree_sitter::Node, source: &str) -> Option<String> {
    if let Some(child) = node.child_by_field_name("name") {
        let text = &source[child.byte_range()];
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    if node.kind() == "impl_item" {
        let type_node = node.child_by_field_name("type");
        let trait_node = node.child_by_field_name("trait");
        if let (Some(tr), Some(ty)) = (trait_node, type_node) {
            let tr_str = source[tr.byte_range()].trim();
            let ty_str = source[ty.byte_range()].trim();
            return Some(format!("impl {tr_str} for {ty_str}"));
        } else if let Some(ty) = type_node {
            let ty_str = source[ty.byte_range()].trim();
            return Some(format!("impl {ty_str}"));
        }
    }

    if node.kind() == "atx_heading" || node.kind() == "setext_heading" {
        let text = &source[node.byte_range()];
        let cleaned = text.trim().trim_start_matches('#').trim();
        if !cleaned.is_empty() {
            return Some(cleaned.to_string());
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "identifier" | "type_identifier" | "field_identifier" | "property_identifier"
        ) {
            let text = &source[child.byte_range()];
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Fallback outline extraction based on line patterns when Tree-sitter tree is unavailable.
pub fn extract_fallback_symbols(source: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        let line_num = (idx + 1) as u32;
        if let Some(rest) = trimmed.strip_prefix("fn ") {
            let name = rest.split('(').next().unwrap_or("").trim();
            if !name.is_empty() {
                symbols.push(DocumentSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    line: line_num,
                    column: 1,
                    start_byte: 0,
                    end_byte: 0,
                });
            }
        } else if let Some(rest) = trimmed.strip_prefix("def ") {
            let name = rest.split(&['(', ':'][..]).next().unwrap_or("").trim();
            if !name.is_empty() {
                symbols.push(DocumentSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    line: line_num,
                    column: 1,
                    start_byte: 0,
                    end_byte: 0,
                });
            }
        } else if let Some(rest) = trimmed.strip_prefix("struct ") {
            let name = rest.split(&['{', ';', '('][..]).next().unwrap_or("").trim();
            if !name.is_empty() {
                symbols.push(DocumentSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Struct,
                    line: line_num,
                    column: 1,
                    start_byte: 0,
                    end_byte: 0,
                });
            }
        } else if let Some(rest) = trimmed.strip_prefix("class ") {
            let name = rest.split(&['{', ':', '('][..]).next().unwrap_or("").trim();
            if !name.is_empty() {
                symbols.push(DocumentSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    line: line_num,
                    column: 1,
                    start_byte: 0,
                    end_byte: 0,
                });
            }
        }
    }
    symbols
}

/// Extracts symbols from source text, using the compiled-in Tree-sitter grammar
/// matching `file_name`'s extension if available, or line-based fallback otherwise.
#[must_use]
pub fn extract_symbols_from_source(source: &str, file_name: Option<&str>) -> Vec<DocumentSymbol> {
    if let Some(file_name) = file_name
        && let Some(ext) = std::path::Path::new(file_name)
            .extension()
            .and_then(|e| e.to_str())
        && let Some(lang) = crate::LanguageId::from_extension(ext)
    {
        let mut doc = Document::new(lang);
        doc.set_text(source);
        document_symbols(&doc, source)
    } else {
        extract_fallback_symbols(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LanguageId;

    #[test]
    fn extracts_rust_symbols() {
        let mut doc = Document::new(LanguageId::Rust);
        let src = r#"
fn calculate_total(a: i32) -> i32 { a * 2 }

struct Order {
    id: u64,
}

enum Status {
    Pending,
}

trait Processor {
    fn process(&self);
}
"#;
        doc.set_text(src);
        let symbols = document_symbols(&doc, src);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"calculate_total"));
        assert!(names.contains(&"Order"));
        assert!(names.contains(&"Status"));
        assert!(names.contains(&"Processor"));
        assert_eq!(
            symbols
                .iter()
                .find(|s| s.name == "calculate_total")
                .unwrap()
                .kind,
            SymbolKind::Function
        );
        assert_eq!(
            symbols.iter().find(|s| s.name == "Order").unwrap().kind,
            SymbolKind::Struct
        );
    }

    #[test]
    fn extracts_python_symbols() {
        let mut doc = Document::new(LanguageId::Python);
        let src = "def greet(name):\n    pass\n\nclass User:\n    pass\n";
        doc.set_text(src);
        let symbols = document_symbols(&doc, src);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"greet"));
        assert!(names.contains(&"User"));
    }

    #[test]
    fn fallback_symbol_extraction_works_without_tree() {
        let src = "fn fallback_func() {\n}\nstruct FallbackStruct;\n";
        let symbols = extract_fallback_symbols(src);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "fallback_func");
        assert_eq!(symbols[1].name, "FallbackStruct");
    }

    #[test]
    fn extract_symbols_from_source_detects_language() {
        let rs_src = "pub fn compute_sum(a: i32, b: i32) -> i32 { a + b }\n";
        let syms = extract_symbols_from_source(rs_src, Some("math.rs"));
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "compute_sum");
        assert_eq!(syms[0].kind, SymbolKind::Function);

        let py_src = "def calculate_area(r):\n    return 3.14 * r * r\n";
        let py_syms = extract_symbols_from_source(py_src, Some("calc.py"));
        assert_eq!(py_syms.len(), 1);
        assert_eq!(py_syms[0].name, "calculate_area");
    }
}
