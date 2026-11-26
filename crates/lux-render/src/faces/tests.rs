use crate::text_layout::{LineStyle, TextLayoutContext};

const AHEM_TTF: &[u8] = include_bytes!("../../tests/fixtures/ahem.ttf");
/// Space, `e` and U+0323 only.
const MARK_TTF: &[u8] = include_bytes!("../../tests/fixtures/dejavu-mono-mark.ttf");
/// Space and ™ only, family "Ahem Emoji".
const EMOJI_TTF: &[u8] = include_bytes!("../../tests/fixtures/ahem-emoji.ttf");

fn glyph_for(cx: &mut TextLayoutContext, family: &str, text: &str, ch: char) -> u32 {
    let style = LineStyle {
        family,
        font_size: 16.0,
        ligatures: false,
    };
    let layout = cx.layout_line(&style, text, &[]);
    let start = text.find(ch).expect("char in text");
    layout
        .clusters()
        .into_iter()
        .find(|c| c.byte_range.start == start)
        .and_then(|c| c.glyph_id)
        .expect("cluster for char")
}

#[test]
fn a_char_no_face_covers_is_reported_once_then_drawn_from_a_registered_face() {
    let ahem = swash::FontRef::from_index(AHEM_TTF, 0).expect("ahem parses");
    assert_ne!(ahem.charmap().map('É'), 0, "fixture: Ahem covers É");
    let mark = swash::FontRef::from_index(MARK_TTF, 0).expect("mark parses");
    assert_eq!(mark.charmap().map('É'), 0, "fixture: the subset lacks É");

    let mut cx = TextLayoutContext::new();
    let family = cx.register_font(MARK_TTF).expect("mark subset registers");

    assert_eq!(glyph_for(&mut cx, &family, "e É e", 'É'), 0);
    assert_eq!(cx.take_missing_chars(), vec!['É']);
    // Reported once: the next frame shapes the same line again.
    assert_eq!(glyph_for(&mut cx, &family, "e É e", 'É'), 0);
    assert!(cx.take_missing_chars().is_empty());

    assert!(cx.register_fallback_face("ahem.ttf", AHEM_TTF.to_vec()));
    assert!(
        cx.register_fallback_face("ahem.ttf", AHEM_TTF.to_vec()),
        "dedupes by key"
    );
    assert_eq!(cx.fallback_face_keys().collect::<Vec<_>>(), ["ahem.ttf"]);
    assert_ne!(glyph_for(&mut cx, &family, "e É e", 'É'), 0);
    // The primary still draws what it covers.
    assert_eq!(
        glyph_for(&mut cx, &family, "e É e", 'e'),
        u32::from(mark.charmap().map('e'))
    );
    assert!(cx.take_missing_chars().is_empty());
}

#[test]
fn ascii_lines_and_whitespace_are_never_reported() {
    let mut cx = TextLayoutContext::new();
    let family = cx.register_font(MARK_TTF).expect("mark subset registers");
    // Every ASCII letter but `e` is missing from the subset; ASCII lines
    // are skipped because the real primary face always covers them.
    glyph_for(&mut cx, &family, "abc", 'a');
    // U+3000 ideographic space is not ASCII, is whitespace, and is missing.
    glyph_for(&mut cx, &family, "e\u{3000}e", '\u{3000}');
    assert!(cx.take_missing_chars().is_empty());
}

#[test]
fn a_coverage_face_arriving_forgets_earlier_reports() {
    let mut cx = TextLayoutContext::new();
    let family = cx.register_font(MARK_TTF).expect("mark subset registers");
    assert!(!cx.has_coverage_fonts());
    glyph_for(&mut cx, &family, "eÉ", 'É');
    assert!(cx.register_coverage_font(AHEM_TTF, &[*b"Latn"], false));
    assert!(cx.has_coverage_fonts());
    assert!(
        cx.take_missing_chars().is_empty(),
        "pending reports from before the coverage face are dropped"
    );
}

#[test]
fn bytes_that_are_not_a_font_are_not_registered() {
    let mut cx = TextLayoutContext::new();
    assert!(!cx.register_fallback_face("junk", b"not a font".to_vec()));
}

fn font_for(cx: &mut TextLayoutContext, family: &str, text: &str, ch: char) -> u64 {
    let style = LineStyle {
        family,
        font_size: 16.0,
        ligatures: false,
    };
    let layout = cx.layout_line(&style, text, &[]);
    let start = text.find(ch).expect("char in text");
    layout
        .clusters()
        .into_iter()
        .find(|c| c.byte_range.start == start)
        .expect("cluster for char")
        .font_id
}

/// ™ is an emoji code point with text presentation by default, like ☀ and
/// ❤. A dynamic face that happens to map it must not beat the color emoji
/// face, or it draws in monochrome.
#[test]
fn the_emoji_face_wins_emoji_code_points_over_a_dynamic_face() {
    let mark = swash::FontRef::from_index(MARK_TTF, 0).expect("mark parses");
    assert_eq!(mark.charmap().map('™'), 0, "fixture: the subset lacks ™");
    let mut cx = TextLayoutContext::new();
    let family = cx.register_font(MARK_TTF).expect("mark subset registers");
    let primary = font_for(&mut cx, &family, "e™e", 'e');

    assert!(cx.register_fallback_face("symbols.ttf", AHEM_TTF.to_vec()));
    let dynamic = font_for(&mut cx, &family, "e™e", '™');
    assert_ne!(
        dynamic, primary,
        "the dynamic face draws ™ with no emoji face"
    );

    assert!(cx.register_coverage_font(EMOJI_TTF, &[], true));
    let emoji = font_for(&mut cx, &family, "e™e", '™');
    assert_ne!(emoji, dynamic, "the emoji face comes before dynamic faces");
    assert_ne!(emoji, primary);
    // The primary still draws what it covers.
    assert_eq!(font_for(&mut cx, &family, "e™e", 'e'), primary);
}

#[test]
fn the_family_list_is_built_once_per_primary_and_face_set() {
    let mut cx = TextLayoutContext::new();
    let family = cx.register_font(MARK_TTF).expect("mark subset registers");
    assert!(cx.register_fallback_face("ahem.ttf", AHEM_TTF.to_vec()));
    cx.fallback.prepare(&family);
    let first = cx.fallback.list.as_ptr();
    cx.fallback.prepare(&family);
    assert_eq!(cx.fallback.list.as_ptr(), first, "no rebuild per line");
    let parley::FontFamily::List(list) = cx.fallback.family(&family) else {
        panic!("a list once a dynamic face exists");
    };
    assert_eq!(list.len(), 3, "primary, emoji, one dynamic face");
    assert!(matches!(
        list[1],
        parley::FontFamilyName::Generic(fontique::GenericFamily::Emoji)
    ));
}
