//! Pure application state. No window, renderer, terminal engine or filesystem is
//! needed to validate layouts or run commands. Runtime owners execute returned
//! effects and deliver generation-tagged completions back to the controller.

mod controller;
mod ids;
mod layout;
mod workspace;

pub use controller::{Command, Completion, Controller, Destination, Effect};
pub use ids::{PaneId, SessionGeneration, SplitId, WorkspaceId};
pub use layout::{Axis, Edge, Layout};
pub use workspace::{Error, Lifecycle, Limits, Model, Pane, PaneSpec, Workspace, WorkspaceSpec};
