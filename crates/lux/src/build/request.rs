//! `Cmd+B` and Build With need to know which marker files (`Cargo.toml`,
//! `package.json`, ...) exist before they can list builds. That probe
//! touches the disk, so it runs on a worker and the request finishes on
//! the UI thread when the result lands (CLAUDE.md rule 6).

use lux_platform::PlatformContext;

use super::detect::Markers;
use super::{BuildPickerState, select};
use crate::app::App;
use crate::ui_io::Done;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Request {
    /// `Cmd+B`: run the automatic choice.
    Build,
    /// Build With: list every candidate in the picker.
    BuildWith,
}

/// A finished probe, applied on the UI thread.
pub(crate) struct Detected {
    generation: u64,
    request: Request,
    markers: Markers,
}

/// Probes the marker files for the active file and folder on a worker.
/// A newer request supersedes an older one still in flight.
pub(crate) fn start(app: &mut App, ctx: Option<&dyn PlatformContext>, request: Request) {
    app.ui.build.detect_generation += 1;
    let generation = app.ui.build.detect_generation;
    let root = app.workspace_root();
    let file = app.documents.path.clone();
    crate::ui_io::spawn(app, ctx, move || {
        let markers = Markers::probe(root.as_deref(), file.as_deref());
        Some(Done::BuildDetected(Box::new(Detected {
            generation,
            request,
            markers,
        })))
    });
}

pub(crate) fn finish(app: &mut App, ctx: &dyn PlatformContext, detected: Detected) {
    if detected.generation != app.ui.build.detect_generation {
        return;
    }
    let markers = detected.markers;
    if markers.root != app.workspace_root() || markers.file != app.documents.path {
        // The active file or folder changed while the probe ran.
        start(app, Some(ctx), detected.request);
        return;
    }
    match detected.request {
        Request::Build => run_automatic(app, ctx, &markers),
        Request::BuildWith => open_picker(app, &markers),
    }
    app.scheduler.invalidate_all();
}

fn run_automatic(app: &mut App, ctx: &dyn PlatformContext, markers: &Markers) {
    match select::automatic(app, markers) {
        Some(variant) => {
            app.execute_build_variant(variant, Some(ctx));
        }
        None => app.ui.announcement = Some("No build system found for this project".into()),
    }
}

fn open_picker(app: &mut App, markers: &Markers) {
    let variants = select::candidates(app, markers);
    if variants.is_empty() {
        app.ui.announcement = Some("No build system variants found".into());
        return;
    }
    app.ui.build.picker = Some(BuildPickerState {
        variants,
        query: crate::field::QueryField::new(),
        selected: 0,
    });
}
