//! The unified Lux desktop binary — the first real assembly of
//! `lux-platform` + `lux-render` + `lux-ui` into one running application,
//! closing out Phase 2 (see `docs/receipts/phase-02-lux-app-binario-unificado.md`).

mod app;
mod document;
mod font;
mod input;
mod presentation;
mod render;
mod scale;

fn main() {
    lux_pack::install_panic_hook();
    let (path, font_bytes) = font::load_system_monospace_font().expect(
        "no known system monospace font was found — see \
         crates/lux/src/font.rs's candidate list",
    );
    println!("Lux: using system font {path}");
    let mut app = app::App::new(font_bytes);
    let mut args = std::env::args_os().skip(1);
    app.documents.initial_path = args.next().map(std::path::PathBuf::from);
    if args.next().is_some() {
        eprintln!("Usage: lux [file]");
        std::process::exit(2);
    }
    lux_platform::desktop::run(app).expect("desktop event loop failed");
}
