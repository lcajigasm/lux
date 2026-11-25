//! Minimal-gap alignment of a query inside one candidate path, with the
//! positions it picked. `rank_paths` ranks by this alignment, so the
//! positions it returns are exactly the characters the ranking judged —
//! the ones a result list highlights.

/// The query as the alignment sees it: whitespace dropped, lowercased
/// unless it carries an uppercase letter (smart case, like `nucleo`).
pub(super) struct Needle {
    chars: Vec<char>,
    sensitive: bool,
}

impl Needle {
    pub(super) fn new(query: &str) -> Option<Self> {
        let sensitive = query.chars().any(char::is_uppercase);
        let chars: Vec<char> = query
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .map(|ch| if sensitive { ch } else { fold(ch) })
            .collect();
        (!chars.is_empty()).then_some(Self { chars, sensitive })
    }

    fn eq(&self, hay: char, needle: char) -> bool {
        if self.sensitive {
            hay == needle
        } else {
            fold(hay) == needle
        }
    }
}

fn fold(ch: char) -> char {
    ch.to_lowercase().next().unwrap_or(ch)
}

/// One way of laying the query over a path, and how good it is.
pub(super) struct Alignment {
    /// Matched character (not byte) indices into the whole path, ascending.
    pub(super) indices: Vec<u32>,
    /// Unmatched characters inside the matched span.
    gap: u32,
    contiguous: bool,
    /// Every matched character sits on a word boundary (`foo-bar`, `/`, camel).
    acronym: bool,
    /// The first matched character starts a word.
    prefix: bool,
    /// Every matched character is in the file name, not the directory.
    basename: bool,
}

/// Score of one skipped character. `nucleo`'s score is clamped below one
/// step, so it only orders matches the tiers above call equal.
pub const GAP_STEP: u32 = 1_000;
/// Gaps longer than this rank as this long.
pub const MAX_GAP: u32 = 9_999;
/// Largest `nucleo` score kept as a tie-break.
pub(super) const NUCLEO_MAX: u32 = GAP_STEP - 1;
/// One shape/word-start/file-name band. Larger than every gap rank plus
/// any bonus worth up to [`MAX_GAP`] skipped characters, so nothing added
/// below a band, recency included, can cross it.
const BAND_STEP: u32 = (MAX_GAP + 1) * GAP_STEP * 2;

/// Packs the documented tiers into one number, most significant first:
/// shape (contiguous 2, acronym 1, scattered 0), word start, file name,
/// fewer skipped characters, then `nucleo`. 11 bands × `BAND_STEP` fits
/// in a `u32`.
pub(super) fn composite(shape: u32, prefix: bool, basename: bool, gap: u32, nucleo: u32) -> u32 {
    let band = shape.min(2) * 4 + u32::from(prefix) * 2 + u32::from(basename);
    let gap_rank = (MAX_GAP - gap.min(MAX_GAP)) * GAP_STEP;
    band * BAND_STEP + gap_rank + nucleo.min(NUCLEO_MAX)
}

impl Alignment {
    pub(super) fn adjust(&self, nucleo: u32) -> u32 {
        let shape = if self.contiguous {
            2
        } else if self.acronym {
            1
        } else {
            0
        };
        composite(shape, self.prefix, self.basename, self.gap, nucleo)
    }
}

/// The best alignment of `needle` in `haystack`: the tighter of the best
/// whole-path alignment and the best alignment inside the file name.
pub(super) fn best(haystack: &str, needle: &Needle) -> Option<Alignment> {
    let h: Vec<char> = haystack.chars().collect();
    let base = h
        .iter()
        .rposition(|ch| matches!(ch, '/' | '\\'))
        .map_or(0, |sep| sep + 1);
    let mut full = align(&h, 0, needle)?;
    full.basename = full.indices[0] as usize >= base;
    if base == 0 || full.basename {
        return Some(full);
    }
    match align(&h, base, needle) {
        Some(mut name) => {
            name.basename = true;
            Some(if name.adjust(0) >= full.adjust(0) {
                name
            } else {
                full
            })
        }
        None => Some(full),
    }
}

/// Dynamic program over `h[start..]`: for each query character and each
/// haystack position, the smallest gap of any alignment of the query so far
/// ending there. Word-boundary checks look at the whole path, so the first
/// character of a file name counts as a word start.
fn align(h: &[char], start: usize, needle: &Needle) -> Option<Alignment> {
    let q = &needle.chars;
    let n = h.len();
    const NONE: u32 = u32::MAX / 4;
    let mut gap = vec![NONE; n];
    let mut acronym = vec![false; n];
    let mut prefix = vec![false; n];
    for j in start..n {
        if needle.eq(h[j], q[0]) {
            gap[j] = 0;
            acronym[j] = on_boundary(h, j);
            prefix[j] = acronym[j];
        }
    }
    // from[k][j]: where query character k - 1 sits when character k is at j.
    let mut from: Vec<Vec<u32>> = Vec::with_capacity(q.len().saturating_sub(1));
    for &ch in q.iter().skip(1) {
        let mut best: Option<(i64, bool, bool, usize)> = None;
        let mut next_gap = vec![NONE; n];
        let mut next_acronym = vec![false; n];
        let mut next_prefix = vec![false; n];
        let mut back = vec![u32::MAX; n];
        for j in start..n {
            if let Some((key, acro, pre, at)) = best
                && needle.eq(h[j], ch)
            {
                next_gap[j] = (key + j as i64 - 1) as u32;
                next_acronym[j] = acro && on_boundary(h, j);
                next_prefix[j] = pre;
                back[j] = at as u32;
            }
            if gap[j] < NONE {
                let key = gap[j] as i64 - j as i64;
                let take = match best {
                    None => true,
                    Some((prev, prev_acro, prev_pre, _)) => {
                        key < prev
                            || (key == prev && (acronym[j], prefix[j]) > (prev_acro, prev_pre))
                    }
                };
                if take {
                    best = Some((key, acronym[j], prefix[j], j));
                }
            }
        }
        gap = next_gap;
        acronym = next_acronym;
        prefix = next_prefix;
        from.push(back);
    }
    // Smallest gap; among equals prefer word shapes, then the rightmost end,
    // which sits closest to the file name.
    let end = (start..n)
        .filter(|&j| gap[j] < NONE)
        .min_by_key(|&j| (gap[j], !acronym[j], !prefix[j], std::cmp::Reverse(j)))?;
    let mut indices = vec![0u32; q.len()];
    let mut at = end as u32;
    for k in (0..q.len()).rev() {
        indices[k] = at;
        if k > 0 {
            at = from[k - 1][at as usize];
        }
    }
    Some(Alignment {
        indices,
        gap: gap[end],
        contiguous: gap[end] == 0,
        acronym: acronym[end],
        prefix: prefix[end],
        basename: false,
    })
}

fn on_boundary(chars: &[char], index: usize) -> bool {
    let Some(prev) = index.checked_sub(1).map(|prev| chars[prev]) else {
        return true;
    };
    let cur = chars[index];
    matches!(prev, '/' | '\\' | '-' | '_' | '.' | ' ' | ':')
        || (prev.is_lowercase() && cur.is_uppercase())
}

#[cfg(test)]
mod tests {
    use super::{BAND_STEP, GAP_STEP, MAX_GAP, composite};

    #[test]
    fn the_largest_score_fits_in_a_u32() {
        let top = u64::from(composite(2, true, true, 0, u32::MAX));
        assert!(top + u64::from(MAX_GAP * GAP_STEP) < u64::from(u32::MAX));
        assert!(u64::from(BAND_STEP) * 11 < u64::from(u32::MAX));
    }

    #[test]
    fn each_tier_outweighs_every_tier_below_it() {
        // Shape beats word start, file name, gap, and nucleo combined.
        assert!(composite(2, false, false, MAX_GAP, 0) > composite(1, true, true, 0, u32::MAX));
        assert!(composite(1, false, false, MAX_GAP, 0) > composite(0, true, true, 0, u32::MAX));
        // Word start beats file name, gap, and nucleo.
        assert!(composite(0, true, false, MAX_GAP, 0) > composite(0, false, true, 0, u32::MAX));
        // File name beats gap and nucleo.
        assert!(composite(0, false, true, MAX_GAP, 0) > composite(0, false, false, 0, u32::MAX));
        // One skipped character fewer beats any nucleo score.
        assert!(composite(0, false, false, 1, 0) > composite(0, false, false, 2, u32::MAX));
    }

    #[test]
    fn a_bonus_of_k_gap_steps_lifts_over_at_most_k_skipped_characters() {
        let bonus = 5 * GAP_STEP;
        let best = composite(0, true, true, 1, 0);
        // Five more skipped characters plus the bonus ties on gap; nucleo
        // decides. Six more stays below whatever nucleo says.
        assert!(composite(0, true, true, 6, u32::MAX) + bonus > best);
        assert!(composite(0, true, true, 7, u32::MAX) + bonus < best);
        // No bonus below a band crosses into the band above.
        let gap_bonus = MAX_GAP * GAP_STEP;
        assert!(
            composite(0, true, false, 0, u32::MAX) + gap_bonus
                < composite(0, true, true, MAX_GAP, 0)
        );
    }
}
