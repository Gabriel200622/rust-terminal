//! Widgets consume presentation data and emit targeted actions. Only the controller
//! may change durable workspace state; renderer caches remain desktop-owned.
pub mod chrome;
pub mod controls;
pub mod dialogs;
pub mod helpers;
pub mod palette;
pub mod preferences;
pub mod search;
pub mod workspace;
use crate::{config::Config, terminal::Cache};
use pace_model::{Axis, PaneId, SplitId, WorkspaceId};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Close {
    Pane(PaneId),
    Workspace(WorkspaceId),
    App,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OverlayState {
    #[default]
    None,
    Settings,
    Palette,
    Rename(WorkspaceId),
    ConfirmClose(Close),
}
#[derive(Default)]
pub struct UiState {
    pub overlay: OverlayState,
    pub palette_query: String,
    /// Highlighted command; reset whenever the query changes.
    pub palette_selected: usize,
    pub rename_name: String,
    /// A dialog field should take keyboard focus on its first frame.
    pub overlay_focus: bool,
    pub error: Option<String>,
    pub search_open: bool,
    pub search: String,
    pub search_error: Option<String>,
    pub search_focus: bool,
    pub zoomed: bool,
    /// Width shown while the sidebar edge is dragged; saved on release.
    pub sidebar_drag: Option<f32>,
}
#[derive(Clone)]
pub enum Action {
    Split(PaneId, Axis),
    ClosePane(PaneId),
    CloseWorkspace(WorkspaceId),
    SelectWorkspace(WorkspaceId),
    Focus(PaneId),
    Ratio(SplitId, f32),
    Rename(WorkspaceId),
    New,
    Settings,
    Palette,
    ToggleSidebar,
    SidebarWidth(f32),
    Find,
    SearchChanged,
    FindNext { reverse: bool },
    CloseSearch,
    Clear(PaneId),
    Restart(PaneId),
    Copy(PaneId),
    Paste(PaneId),
    Zoom,
    WindowClose,
    Create(PathBuf, Option<String>),
    SetName(WorkspaceId, String),
    Preferences(Config),
    Confirm(Close),
    CancelClose,
    CloseOverlay,
    DismissError,
    Resize(PaneId, crate::terminal_view::geometry::ResizeRequest),
    Selection(PaneId, crate::terminal_view::SelectionInteraction),
    ScrollBottom(PaneId),
}
#[derive(Clone)]
pub struct WorkspaceView {
    pub id: WorkspaceId,
    pub name: String,
    pub cwd: PathBuf,
    pub panes: usize,
    pub running: bool,
}
pub struct PaneRender {
    pub preedit: String,
    pub id: PaneId,
    pub cache: Cache,
    pub mouse_button: Option<u8>,
    pub mouse_cell: Option<(u16, u16, Option<u8>)>,
}
impl PaneRender {
    pub fn new(id: PaneId) -> Self {
        Self {
            id,
            preedit: String::new(),
            cache: Cache::default(),
            mouse_button: None,
            mouse_cell: None,
        }
    }
}
