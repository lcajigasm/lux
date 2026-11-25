//! Every overlay list lights the positions its own filter matched, carried
//! on the row: fuzzy-ranked lists the matcher's alignment, substring
//! filters the substring. Nothing re-scans the title at paint time.

use std::ops::Range;
use std::path::PathBuf;

use crate::field::QueryField;

fn lit<'a>(title: &'a str, hits: &[Range<usize>]) -> Vec<&'a str> {
    hits.iter().map(|span| &title[span.clone()]).collect()
}

#[test]
fn theme_picker_lights_the_fuzzy_positions_not_a_substring() {
    let mut picker = crate::theme_picker::ThemePickerState::new("Monokai");
    picker.query = QueryField::from_text("mki");
    let ranked = picker.ranked();
    let (name, hits) = ranked
        .iter()
        .find(|(name, _)| *name == "Monokai")
        .expect("Monokai matches `mki`");
    assert_eq!(lit(name, hits), ["M", "k", "i"]);
    assert_eq!(hits, &vec![0..1, 4..5, 6..7]);
}

#[test]
fn language_picker_lights_the_fuzzy_positions() {
    let mut picker = crate::language_picker::LanguagePickerState::new();
    picker.query = QueryField::from_text("jscr");
    let ranked = picker.ranked();
    let ((_, name), hits) = ranked
        .iter()
        .find(|((_, name), _)| *name == "JavaScript")
        .expect("JavaScript matches `jscr`");
    assert_eq!(lit(name, hits).concat().to_lowercase(), "jscr");
}

fn build_picker(query: &str) -> crate::build::BuildPickerState {
    crate::build::BuildPickerState {
        variants: vec![crate::build::BuildVariant {
            name: "Release".into(),
            command: "make".into(),
            args: Vec::new(),
            file_regex: None,
        }],
        query: QueryField::from_text(query),
        selected: 0,
    }
}

#[test]
fn build_picker_lights_the_part_of_the_match_in_the_name() {
    let picker = build_picker("rel");
    let ranked = picker.ranked();
    assert_eq!(lit(&ranked[0].0.name, &ranked[0].1), ["R", "e", "l"]);
    // `mak` matches only the command, which the title does not show.
    let picker = build_picker("mak");
    let ranked = picker.ranked();
    assert_eq!(ranked.len(), 1);
    assert!(ranked[0].1.is_empty());
}

#[test]
fn snippet_picker_lights_the_part_of_the_match_in_the_name() {
    let mut picker = crate::snippets::picker::SnippetPickerState::new(vec![
        crate::snippets::registry::Snippet {
            name: "For Loop".into(),
            trigger: "for".into(),
            content: String::new(),
            scope: None,
            description: String::new(),
        },
    ]);
    picker.query = QueryField::from_text("loop");
    let ranked = picker.ranked();
    assert_eq!(lit(&ranked[0].0.name, &ranked[0].1).concat(), "Loop");
}

#[test]
fn project_picker_lights_the_match_inside_the_folder_name() {
    let picker = crate::project::ProjectSwitchState {
        projects: vec![PathBuf::from("/home/me/work/lux")],
        query: QueryField::from_text("lux"),
        selected: 0,
    };
    let ranked = picker.ranked();
    assert_eq!(ranked[0].1, vec![0..1, 1..2, 2..3]);
}

#[test]
fn branch_picker_lights_the_substring_and_not_the_create_row() {
    let mut picker =
        crate::git::branch_picker::BranchPicker::new(vec!["feature/login".into()], false);
    picker.query = QueryField::from_text("LOG");
    let rows = picker.rows_with_hits();
    let (create, create_hits) = &rows[0];
    assert!(create.starts_with("Create branch"));
    assert!(create_hits.is_empty());
    let (name, hits) = &rows[1];
    assert_eq!(name, "feature/login");
    assert_eq!(lit(name, hits), ["log"]);
}
