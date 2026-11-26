use super::*;

fn query(q: &str) -> Regex {
    Regex::new(q).unwrap()
}

#[test]
fn a_results_buffer_is_split_into_header_number_and_text_runs() {
    let text = crate::find_results::format_find_results(
        "counts",
        Some(std::path::Path::new("/w")),
        &[lux_search::find_in_files::SearchMatch {
            path: "/w/src/main.rs".into(),
            line_number: 17,
            line: "    let counts = count_words(counts);".into(),
        }],
        13,
    );
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "Searching files for \"counts\"");
    assert_eq!(lines[2], "src/main.rs:", "header is relative to the root");
    let q = query("counts");
    assert_eq!(decorate(lines[0], Some(&q)).role_at(0), Role::Meta);
    assert_eq!(decorate(lines[2], Some(&q)).role_at(0), Role::Header);
    let hit_line = decorate(lines[3], Some(&q));
    assert_eq!(hit_line.role_at(2), Role::Number);
    assert_eq!(hit_line.role_at(lines[3].find(':').unwrap()), Role::Number);
    assert_eq!(hit_line.role_at(lines[3].len() - 1), Role::Text);
    assert_eq!(hit_line.hits.len(), 2);
    for hit in &hit_line.hits {
        assert_eq!(&lines[3][hit.clone()], "counts");
    }
    let summary = lines.last().unwrap();
    assert_eq!(decorate(summary, Some(&q)).role_at(0), Role::Meta);
}

#[test]
fn the_gutter_never_counts_as_a_hit() {
    // A query that matches the line number must not light the gutter.
    let line = "    17: x = 17;";
    let decor = decorate(line, Some(&query("17")));
    assert_eq!(decor.hits.len(), 1);
    assert!(decor.hits[0].start > line.find(':').unwrap());
}

#[test]
fn a_match_line_ending_in_a_colon_is_not_a_header() {
    let decor = decorate("    12: impl Foo for Bar:", None);
    assert_eq!(decor.role_at(2), Role::Number);
    assert_eq!(decor.role_at(10), Role::Text);
}

#[test]
fn a_restored_tab_reads_its_query_from_the_header_and_caches_it() {
    let mut nav = FindResultsNav::default();
    let first = "Searching files for \"a.b\"";
    assert_eq!(header_query(first), Some("a.b"));
    let regex = matcher(&mut nav, first).expect("compiles");
    assert!(regex.is_match("axb"));
    assert_eq!(nav.highlight.as_ref().unwrap().0, "a.b");
    // A run's own matcher (with its case flag) is kept for the same query.
    nav.highlight = Some(run_matcher("A.B", true));
    let regex = matcher(&mut nav, "Searching files for \"A.B\"").unwrap();
    assert!(regex.is_match("axb"), "case-insensitive run matcher kept");
    assert_eq!(header_query("plain text"), None);
}
