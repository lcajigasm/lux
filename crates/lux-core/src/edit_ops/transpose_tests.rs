use super::*;
use crate::edit_ops::apply_to_each;

fn buffer(text: &str) -> Buffer {
    Buffer::from_bytes(text.as_bytes()).unwrap()
}

fn range(buffer: &Buffer, from: usize, to: usize) -> Selection {
    Selection::new(
        Position::from_char(buffer, from),
        Position::from_char(buffer, to),
    )
}

fn run(text: &str, ranges: &[(usize, usize)]) -> (String, Vec<(usize, usize)>) {
    let mut buffer = buffer(text);
    let selections = ranges
        .iter()
        .map(|&(from, to)| range(&buffer, from, to))
        .collect();
    let cursors = MultiCursor::from_cursors(selections);
    let op = transpose_op(&buffer, &cursors);
    let (_, after) = apply_to_each(&mut buffer, &cursors, op);
    let ranges = after
        .cursors()
        .iter()
        .map(|s| (s.anchor.char, s.head.char))
        .collect();
    (buffer.text(), ranges)
}

#[test]
fn caret_between_two_chars_swaps_them_and_advances() {
    assert_eq!(run("ab", &[(1, 1)]), ("ba".into(), vec![(2, 2)]));
}

#[test]
fn caret_at_line_end_swaps_the_two_chars_before_it() {
    assert_eq!(run("abc\nxy", &[(3, 3)]), ("acb\nxy".into(), vec![(3, 3)]));
    assert_eq!(run("ab", &[(2, 2)]), ("ba".into(), vec![(2, 2)]));
}

#[test]
fn caret_never_reaches_across_a_line_break() {
    // Line start: nothing before it on the line.
    assert_eq!(run("ab\ncd", &[(3, 3)]).0, "ab\ncd");
    // One char on the line, caret at its end.
    assert_eq!(run("a\nb", &[(1, 1)]).0, "a\nb");
    assert_eq!(run("ab", &[(0, 0)]).0, "ab");
}

#[test]
fn caret_swaps_whole_grapheme_clusters() {
    let text = "e\u{0301}x";
    assert_eq!(run(text, &[(2, 2)]), ("xe\u{0301}".into(), vec![(3, 3)]));
}

#[test]
fn every_caret_transposes_independently() {
    assert_eq!(
        run("ab\ncd\n", &[(1, 1), (4, 4)]),
        ("ba\ndc\n".into(), vec![(2, 2), (5, 5)])
    );
}

#[test]
fn several_selections_rotate_their_text_forward() {
    let (text, ranges) = run("one, three, two", &[(0, 3), (5, 10), (12, 15)]);
    assert_eq!(text, "two, one, three");
    assert_eq!(ranges, vec![(0, 3), (5, 8), (10, 15)]);
}

#[test]
fn empty_carets_stay_put_while_selections_rotate() {
    let (text, _) = run("a b c", &[(0, 1), (2, 2), (4, 5)]);
    assert_eq!(text, "c b a");
}

#[test]
fn a_selection_over_two_lines_swaps_them() {
    let (text, ranges) = run("first\nsecond\nthird", &[(2, 8)]);
    assert_eq!(text, "second\nfirst\nthird");
    assert_eq!(ranges, vec![(0, 12)]);
    // Two whole lines, ending at the start of the third.
    assert_eq!(run("a\nbb\nc", &[(0, 5)]).0, "bb\na\nc");
}

#[test]
fn a_selection_over_two_words_swaps_them() {
    let (text, ranges) = run("let foo = bar;", &[(4, 13)]);
    assert_eq!(text, "let bar = foo;");
    assert_eq!(ranges, vec![(4, 13)]);
}

#[test]
fn a_selected_word_trades_places_with_the_next_word() {
    let (text, ranges) = run("alpha beta gamma", &[(0, 5)]);
    assert_eq!(text, "beta alpha gamma");
    assert_eq!(ranges, vec![(5, 10)]);
}

#[test]
fn a_selected_last_word_trades_places_with_the_previous_word() {
    let (text, ranges) = run("alpha beta", &[(10, 6)]);
    assert_eq!(text, "beta alpha");
    // The selection follows "beta" and keeps its direction.
    assert_eq!(ranges, vec![(4, 0)]);
}

#[test]
fn a_lone_word_with_no_neighbour_is_left_alone() {
    assert_eq!(run("solo\n", &[(0, 4)]).0, "solo\n");
    // Three lines is neither a line swap nor a word swap.
    assert_eq!(run("a\nb\nc", &[(0, 5)]).0, "a\nb\nc");
}

#[test]
fn transpose_is_one_undoable_transaction() {
    let mut buffer = buffer("ab cd");
    let cursors = MultiCursor::from_cursors(vec![range(&buffer, 1, 1), range(&buffer, 4, 4)]);
    let op = transpose_op(&buffer, &cursors);
    let (transaction, _) = apply_to_each(&mut buffer, &cursors, op);
    assert_eq!(buffer.text(), "ba dc");
    transaction.undo(&mut buffer);
    assert_eq!(buffer.text(), "ab cd");
}
