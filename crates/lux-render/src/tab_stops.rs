//! A tab occupies the columns up to the next stop of `editor.tabWidth`
//! (default 4). The font's tab glyph is one cell and is often `.notdef`,
//! so painting it puts a box somewhere other than the caret.

use crate::text_layout::ClusterGeometry;

const TAB_COLUMNS: u32 = 4;

pub(crate) fn apply(clusters: &mut [(ClusterGeometry, bool)]) {
    if clusters.is_empty() {
        return;
    }
    let cell = clusters
        .iter()
        .find(|(cluster, is_tab)| !is_tab && cluster.advance > 0.0)
        .or_else(|| clusters.iter().find(|(cluster, _)| cluster.advance > 0.0))
        .map(|(cluster, _)| cluster.advance);
    let Some(cell) = cell else {
        return;
    };
    let mut x = clusters[0].0.x;
    let mut column = 0u32;
    for (cluster, is_tab) in clusters.iter_mut() {
        cluster.x = x;
        if *is_tab {
            let cols = TAB_COLUMNS - (column % TAB_COLUMNS);
            cluster.advance = cell * cols as f32;
            cluster.glyph_id = None;
            cluster.glyph_dx = 0.0;
            cluster.glyph_dy = 0.0;
            cluster.extra_glyphs.clear();
            column += cols;
        } else {
            let cols = ((cluster.advance / cell).round() as u32).max(1);
            column += cols;
        }
        x += cluster.advance;
    }
}
