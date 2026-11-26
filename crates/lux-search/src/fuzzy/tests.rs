use super::{char_spans, char_spans_in, rank_by, rank_paths, substring_span};
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher};

#[test]
fn drops_non_matching_candidates() {
    let results = rank_paths("goto", ["src/goto_anything.rs", "src/theme.rs"]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].item, "src/goto_anything.rs");
}

#[test]
fn ranks_path_boundary_match_above_mid_segment_match() {
    // "ui" matches right after a `/` in the first path, and mid-segment
    // (inside "build") in the second — the path-boundary bonus from
    // `Config::match_paths` should rank the former higher.
    let results = rank_paths("ui", ["src/ui/mod.rs", "src/builder.rs"]);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].item, "src/ui/mod.rs");
    assert!(results[0].score > results[1].score);
}

#[test]
fn is_case_insensitive_for_lowercase_queries() {
    let results = rank_paths("readme", ["README.md"]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].item, "README.md");
}

#[test]
fn respects_case_for_mixed_case_queries() {
    // `CaseMatching::Smart`: a query with an uppercase letter only
    // matches haystacks with matching case.
    let results = rank_paths("README", ["readme.md", "README.md"]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].item, "README.md");
}

#[test]
fn empty_query_returns_all_candidates_unscored_in_order() {
    let results = rank_paths("", ["b.rs", "a.rs"]);
    let rows: Vec<(&str, u32, usize)> = results
        .iter()
        .map(|r| (r.item, r.score, r.indices.len()))
        .collect();
    assert_eq!(rows, vec![("b.rs", 0, 0), ("a.rs", 0, 0)]);
}

#[test]
fn allows_gaps_between_matched_characters() {
    // Fuzzy, not substring: "gan" matches "goto_anything.rs" via
    // g-o-t-o-_-a...n... with gaps, not as a contiguous run.
    let results = rank_paths("gan", ["goto_anything.rs"]);
    assert_eq!(results.len(), 1);
}

#[test]
fn rank_by_extracts_the_match_key_and_keeps_item_identity() {
    #[derive(Debug, PartialEq, Eq)]
    struct Labeled(u32, &'static str);
    let items = [Labeled(1, "Format Document"), Labeled(2, "Format Document")];
    let results = rank_by("format", &items, |i| i.1);
    // Two items sharing a title are both kept, as themselves (not
    // collapsed or confused) — the reason `rank_by` exists instead of
    // reusing `rank_paths` for non-path candidates.
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].item.0, 1);
    assert_eq!(results[1].item.0, 2);
}

#[test]
fn query_mon_on_the_compare_fixtures_ranks_monokai_first_and_keeps_the_rest() {
    let files = [
        "characters.txt",
        "monokai.rs",
        "sample-latin1.bin",
        "sample-utf16-be-bom.bin",
        "sample-utf16-be-nobom.bin",
        "sample-utf16-le-bom.bin",
        "sample-utf16-le-nobom.bin",
        "sample-utf8-bom.bin",
        "sample-windows-1252.bin",
    ];
    let pattern = Pattern::parse("mon", CaseMatching::Smart, Normalization::Smart);
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let raw = pattern.match_list(files, &mut matcher).len();
    let results = rank_paths("mon", files);
    assert_eq!(results[0].item, "monokai.rs");
    // Ranking, not filtering: every name `nucleo` matched stays listed.
    assert_eq!(results.len(), raw);
    assert!(results[1..].iter().all(|r| r.item.ends_with(".bin")));
}

#[test]
fn a_tight_match_does_not_hide_a_scattered_one() {
    let results = rank_paths("ctrl", ["src/controller.rs", "src/ctrl.rs"]);
    let names: Vec<&str> = results.iter().map(|r| r.item).collect();
    assert_eq!(names, ["src/ctrl.rs", "src/controller.rs"]);
}

#[test]
fn word_initials_rank_between_a_contiguous_run_and_a_scattered_match() {
    let results = rank_paths(
        "mon",
        ["sample-utf8-bom.bin", "my-other-notes.rs", "monokai.rs"],
    );
    let names: Vec<&str> = results.iter().map(|r| r.item).collect();
    assert_eq!(
        names,
        ["monokai.rs", "my-other-notes.rs", "sample-utf8-bom.bin"]
    );
}

#[test]
fn tier_shape_beats_word_start() {
    // `xab.rs`: contiguous but mid-word. `a_b.rs`: word initials at a
    // word start. Shape is the top tier.
    let results = rank_paths("ab", ["a_b.rs", "xab.rs"]);
    assert_eq!(results[0].item, "xab.rs");
}

#[test]
fn tier_word_start_beats_file_name() {
    // Both contiguous. `lib/x.rs` starts a word in the directory;
    // `src/xlib.rs` is in the file name but mid-word.
    let results = rank_paths("lib", ["src/xlib.rs", "lib/x.rs"]);
    assert_eq!(results[0].item, "lib/x.rs");
}

#[test]
fn tier_file_name_beats_fewer_skipped_characters() {
    // Both scattered from a word start; the directory match skips one
    // character, the file-name match skips four.
    let results = rank_paths("mn", ["mxn/a.rs", "a/mxxxxn.rs"]);
    assert_eq!(results[0].item, "a/mxxxxn.rs");
}

#[test]
fn tier_fewer_skipped_characters_beat_nucleo_score() {
    // Same tiers above the gap; one skipped character beats two whatever
    // `nucleo` thinks, in either input order.
    for files in [["a/mxxn.rs", "a/mxn.rs"], ["a/mxn.rs", "a/mxxn.rs"]] {
        let results = rank_paths("mn", files);
        assert_eq!(results[0].item, "a/mxn.rs");
    }
}

#[test]
fn gaps_are_penalized_when_every_match_is_scattered() {
    let results = rank_paths("gan", ["gxxxxxxxxaxxxxxxxn.rs", "goto_anything.rs"]);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].item, "goto_anything.rs");
    assert!(results[0].score > results[1].score);
}

#[test]
fn rank_by_drops_non_matching_items() {
    let items = [String::from("Save File"), String::from("Close Window")];
    let results = rank_by("save", &items, |s| s.as_str());
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].item.as_str(), "Save File");
}

fn highlighted(path: &str, indices: &[u32]) -> String {
    char_spans(path, indices)
        .into_iter()
        .map(|span| &path[span])
        .collect()
}

#[test]
fn apprs_highlights_app_and_rs_not_the_a_in_crates() {
    let results = rank_paths("apprs", ["crates/lux/src/app.rs"]);
    assert_eq!(results.len(), 1);
    let path = results[0].item;
    // c r a t e s / l u x / s  r  c  /  a  p  p  .  r  s
    // 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20
    assert_eq!(results[0].indices, vec![15, 16, 17, 19, 20]);
    assert_eq!(highlighted(path, &results[0].indices), "apprs");
}

#[test]
fn indices_follow_the_alignment_that_was_ranked() {
    // A greedy left-to-right scan would light `s`, `r`, `c` of the first
    // `src`; the ranked alignment is the contiguous `src` in the file name.
    let results = rank_paths("src", ["src/x/src.rs"]);
    assert_eq!(results[0].indices, vec![6, 7, 8]);
}

#[test]
fn a_match_inside_the_file_name_outranks_the_same_match_in_a_directory() {
    // Both are contiguous word-start runs; the file name wins even though
    // `lib/mod.rs` matches at the very start of the path.
    let results = rank_paths("lib", ["lib/mod.rs", "src/lib.rs"]);
    assert_eq!(results[0].item, "src/lib.rs");
    assert!(results[0].score > results[1].score);
}

#[test]
fn a_scattered_match_inside_the_file_name_outranks_one_spread_over_the_path() {
    // Same gap (one skipped character) either way; only the file name tier
    // separates them, and the directory candidate comes first in input order.
    let results = rank_paths("apprs", ["app/rs/zz.txt", "crates/lux/src/app.rs"]);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].item, "crates/lux/src/app.rs");
    assert!(results[0].score > results[1].score);
}

#[test]
fn file_name_alignment_is_preferred_when_the_path_offers_both() {
    let results = rank_paths("lux", ["crates/lux/src/lux.rs"]);
    assert_eq!(highlighted(results[0].item, &results[0].indices), "lux");
    assert_eq!(results[0].indices, vec![15, 16, 17]);
}

#[test]
fn rank_by_returns_nucleo_positions() {
    let items = [String::from("Goto Line")];
    let results = rank_by("gl", &items, |s| s.as_str());
    assert_eq!(results[0].indices, vec![0, 5]);
}

#[test]
fn char_spans_map_character_indices_to_byte_ranges() {
    assert_eq!(char_spans("añb", &[1, 2]), vec![1..3, 3..4]);
    assert!(char_spans("ab", &[5]).is_empty());
}

#[test]
fn substring_span_is_case_insensitive_and_byte_exact() {
    assert_eq!(substring_span("BuildFrame", "frame"), Some(5..10));
    assert_eq!(substring_span("ÉtÉ", "té"), Some(2..5));
    assert_eq!(substring_span("abc", "x"), None);
    assert_eq!(substring_span("abc", ""), None);
}

#[test]
fn char_spans_in_keeps_and_rebases_only_the_spans_inside_the_window() {
    // "make cargo": indices 0 (m) and 5 (c), window over "cargo".
    assert_eq!(char_spans_in("make cargo", &[0, 5], 5..10), vec![0..1]);
    assert_eq!(char_spans_in("make cargo", &[0, 5], 0..4), vec![0..1]);
}
