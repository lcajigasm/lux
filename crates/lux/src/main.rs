//! The unified Lux desktop binary — the first real assembly of
//! `lux-platform` + `lux-render` + `lux-ui` into one running application,
//! closing out Phase 2 (see `docs/receipts/phase-02-lux-app-binario-unificado.md`).

mod app;
mod font;
mod render;

fn main() {
    let (path, font_bytes) = font::load_system_monospace_font().expect(
        "no known macOS system monospace font was found — see \
         crates/lux/src/font.rs's candidate list",
    );
    println!("Lux: using system font {path}");
    lux_platform::desktop::run(app::App::new(font_bytes)).expect("desktop event loop failed");
}
