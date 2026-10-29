//! UI overlays and chrome state grouping for the Lux desktop application.

use crate::status_bar::StatusBarState;

#[derive(Default)]
pub(crate) struct UiOverlays {
    pub(crate) status_bar: StatusBarState,
    pub(crate) command_palette: Option<crate::palette::CommandPaletteState>,
    pub(crate) goto_anything: Option<crate::goto::GotoState>,
    pub(crate) search_bar: Option<crate::search::SearchBarState>,
    pub(crate) sidebar: Option<crate::sidebar::SidebarState>,
    pub(crate) minimap: crate::minimap::MinimapState,
    pub(crate) project_search: Option<crate::project_search::ProjectSearchState>,
    pub(crate) dialogs: crate::dialog::DialogState,
    pub(crate) session: crate::session::WorkspaceSession,
    pub(crate) git: crate::git::GitState,
    pub(crate) lsp: crate::lsp::LspState,
    pub(crate) term: crate::term::TerminalState,
    pub(crate) plugin: crate::plugin::PluginState,
    pub(crate) announcement: Option<String>,
    pub(crate) settings: crate::settings::SettingsState,
    pub(crate) language_picker: Option<crate::language_picker::LanguagePickerState>,
    pub(crate) split: crate::split::SplitLayout,
    pub(crate) pending_chord: Option<lux_config::Chord>,
    pub(crate) find_results_nav: crate::project_search::FindResultsNav,
    pub(crate) build: crate::build::BuildState,
    pub(crate) project: crate::project::ProjectState,
    pub(crate) theme_picker: Option<crate::theme_picker::ThemePickerState>,
    pub(crate) distraction_free: Option<crate::distraction_free::DistractionFreeState>,
    pub(crate) snippets: crate::snippets::SnippetState,
    pub(crate) macros: crate::macros::MacroState,
}
