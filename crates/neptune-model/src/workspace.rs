use crate::{Layout, PaneId, Remote, WorkspaceGroupId, WorkspaceId};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    UnknownWorkspace(WorkspaceId),
    UnknownWorkspaceGroup(WorkspaceGroupId),
    WorkspaceGroupLimit,
    UnknownPane(PaneId),
    UnknownSplit(crate::SplitId),
    WorkspaceLimit,
    PaneLimit,
    TotalPaneLimit,
    InvalidLayout(&'static str),
    InvalidRatio,
    InvalidIdentity,
    InvalidName,
    InvalidRemote,
    RemoteMismatch,
    IdentityExhausted,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownWorkspace(id) => write!(f, "Workspace {id} does not exist"),
            Self::UnknownWorkspaceGroup(id) => write!(f, "Workspace group {id} does not exist"),
            Self::WorkspaceGroupLimit => f.write_str("Workspace group limit reached"),
            Self::UnknownPane(id) => {
                write!(f, "Pane {id} does not exist in the targeted workspace")
            }
            Self::UnknownSplit(id) => write!(f, "Split {id} does not exist"),
            Self::WorkspaceLimit => f.write_str("Workspace limit reached"),
            Self::PaneLimit => f.write_str("Workspace pane limit reached"),
            Self::TotalPaneLimit => f.write_str("Total pane limit reached"),
            Self::InvalidLayout(reason) => write!(f, "Invalid layout: {reason}"),
            Self::InvalidRatio => f.write_str("Split ratio must be finite and between 0.1 and 0.9"),
            Self::InvalidIdentity => f.write_str("Identities must be nonzero and unique"),
            Self::InvalidName => f.write_str("Name cannot be empty"),
            Self::InvalidRemote => f.write_str(
                "SSH host must be a destination such as user@host, without spaces or a leading dash",
            ),
            Self::RemoteMismatch => f.write_str(
                "A terminal keeps its session, so it cannot move between workspaces on different machines",
            ),
            Self::IdentityExhausted => f.write_str("Identity counter exhausted"),
        }
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub workspaces: usize,
    pub panes_per_workspace: usize,
    pub total_panes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            workspaces: 24,
            panes_per_workspace: 12,
            total_panes: 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lifecycle {
    Starting,
    Running,
    Closing,
    Exited,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    pub(crate) id: PaneId,
    pub(crate) cwd: PathBuf,
    pub(crate) generation: u64,
    pub(crate) lifecycle: Lifecycle,
}
impl Pane {
    pub fn id(&self) -> PaneId {
        self.id
    }
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn lifecycle(&self) -> &Lifecycle {
        &self.lifecycle
    }
}

/// Construction data; model adoption validates all identity and layout invariants.
#[derive(Debug, Clone)]
pub struct PaneSpec {
    pub id: PaneId,
    pub cwd: PathBuf,
}
#[derive(Debug, Clone)]
pub struct WorkspaceSpec {
    pub id: WorkspaceId,
    pub group: Option<WorkspaceGroupId>,
    pub name: String,
    pub cwd: PathBuf,
    /// An SSH destination; validated when the model adopts the workspace.
    pub remote: Option<String>,
    pub panes: Vec<PaneSpec>,
    pub layout: Layout,
    pub active: PaneId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    pub(crate) id: WorkspaceId,
    pub(crate) group: Option<WorkspaceGroupId>,
    pub(crate) name: String,
    pub(crate) cwd: PathBuf,
    pub(crate) remote: Option<Remote>,
    pub(crate) panes: Vec<Pane>,
    pub(crate) layout: Layout,
    pub(crate) active: PaneId,
}
impl Workspace {
    pub fn id(&self) -> WorkspaceId {
        self.id
    }
    pub fn group(&self) -> Option<WorkspaceGroupId> {
        self.group
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    /// The local directory its terminals start in. A remote workspace runs
    /// its SSH client there; the directory on the remote host is not tracked.
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }
    /// Set when every terminal of this workspace runs on another machine.
    pub fn remote(&self) -> Option<&Remote> {
        self.remote.as_ref()
    }
    pub fn panes(&self) -> &[Pane] {
        &self.panes
    }
    pub fn layout(&self) -> &Layout {
        &self.layout
    }
    pub fn active(&self) -> PaneId {
        self.active
    }
    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.iter().find(|pane| pane.id == id)
    }
}

/// Construction data for a sidebar folder. Groups contain workspaces, never groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceGroupSpec {
    pub id: WorkspaceGroupId,
    pub name: String,
    pub collapsed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceGroup {
    pub(crate) id: WorkspaceGroupId,
    pub(crate) name: String,
    pub(crate) collapsed: bool,
}
impl WorkspaceGroup {
    pub fn id(&self) -> WorkspaceGroupId {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn collapsed(&self) -> bool {
        self.collapsed
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub(crate) workspaces: Vec<Workspace>,
    pub(crate) groups: Vec<WorkspaceGroup>,
    pub(crate) active: Option<WorkspaceId>,
    pub(crate) sidebar: bool,
    pub(crate) next_workspace: u64,
    pub(crate) next_group: u64,
    pub(crate) next_pane: u64,
    pub(crate) next_split: u64,
    pub(crate) limits: Limits,
}
impl Default for Model {
    fn default() -> Self {
        Self::new(Limits::default())
    }
}
impl Model {
    pub fn new(limits: Limits) -> Self {
        Self {
            workspaces: Vec::new(),
            groups: Vec::new(),
            active: None,
            sidebar: true,
            next_workspace: 1,
            next_group: 1,
            next_pane: 1,
            next_split: 1,
            limits,
        }
    }
    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }
    pub fn groups(&self) -> &[WorkspaceGroup] {
        &self.groups
    }
    pub fn group(&self, id: WorkspaceGroupId) -> Option<&WorkspaceGroup> {
        self.groups.iter().find(|group| group.id == id)
    }
    pub fn workspace(&self, id: WorkspaceId) -> Option<&Workspace> {
        self.workspaces.iter().find(|workspace| workspace.id == id)
    }
    pub fn active_workspace(&self) -> Option<WorkspaceId> {
        self.active
    }
    pub fn active_pane(&self) -> Option<PaneId> {
        self.active
            .and_then(|id| self.workspace(id))
            .map(|workspace| workspace.active)
    }
    pub fn sidebar(&self) -> bool {
        self.sidebar
    }
    pub fn limits(&self) -> Limits {
        self.limits
    }
    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.workspaces
            .iter()
            .find_map(|workspace| workspace.pane(id))
    }
    pub fn pane_count(&self) -> usize {
        self.workspaces
            .iter()
            .map(|workspace| workspace.panes.len())
            .sum()
    }
    pub fn workspace_for_pane(&self, id: PaneId) -> Option<WorkspaceId> {
        self.workspaces
            .iter()
            .find(|workspace| workspace.pane(id).is_some())
            .map(|workspace| workspace.id)
    }

    /// Filesystem checks are deliberately left to the persistence/runtime owner.
    /// Validation is atomic: a rejected restore never partially changes a model.
    pub fn restore(
        specs: Vec<WorkspaceSpec>,
        active: Option<WorkspaceId>,
        sidebar: bool,
        limits: Limits,
    ) -> Result<Self, Error> {
        Self::restore_grouped(specs, Vec::new(), active, sidebar, limits)
    }

    /// Restores organization along with workspaces, validating membership atomically.
    /// Empty groups are retained; their count shares the workspace resource limit.
    pub fn restore_grouped(
        specs: Vec<WorkspaceSpec>,
        groups: Vec<WorkspaceGroupSpec>,
        active: Option<WorkspaceId>,
        sidebar: bool,
        limits: Limits,
    ) -> Result<Self, Error> {
        if specs.len() > limits.workspaces {
            return Err(Error::WorkspaceLimit);
        }
        let mut model = Self::new(limits);
        if groups.len() > limits.workspaces {
            return Err(Error::WorkspaceGroupLimit);
        }
        let mut group_ids = HashSet::new();
        for group in groups {
            if group.id.get() == 0 || !group_ids.insert(group.id) {
                return Err(Error::InvalidIdentity);
            }
            if group.name.trim().is_empty() {
                return Err(Error::InvalidName);
            }
            model.next_group = model.next_group.max(
                group
                    .id
                    .get()
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?,
            );
            model.groups.push(WorkspaceGroup {
                id: group.id,
                name: group.name,
                collapsed: group.collapsed,
            });
        }
        let mut workspace_ids = HashSet::new();
        let mut pane_ids = HashSet::new();
        let mut split_ids = HashSet::new();
        for spec in specs {
            if let Some(group) = spec.group
                && !group_ids.contains(&group)
            {
                return Err(Error::UnknownWorkspaceGroup(group));
            }
            if spec.id.get() == 0 || !workspace_ids.insert(spec.id) {
                return Err(Error::InvalidIdentity);
            }
            if spec.name.trim().is_empty() {
                return Err(Error::InvalidName);
            }
            let remote = spec.remote.as_deref().map(Remote::parse).transpose()?;
            if spec.panes.is_empty() {
                return Err(Error::InvalidLayout("workspace has no panes"));
            }
            if spec.panes.len() > limits.panes_per_workspace {
                return Err(Error::PaneLimit);
            }
            let mut members = HashSet::new();
            for pane in &spec.panes {
                if pane.id.get() == 0 || !pane_ids.insert(pane.id) {
                    return Err(Error::InvalidIdentity);
                }
                members.insert(pane.id);
                model.next_pane = model.next_pane.max(
                    pane.id
                        .get()
                        .checked_add(1)
                        .ok_or(Error::IdentityExhausted)?,
                );
            }
            if !members.contains(&spec.active) {
                return Err(Error::UnknownPane(spec.active));
            }
            spec.layout.validate(&members, &mut split_ids)?;
            model.next_split = model.next_split.max(
                spec.layout
                    .max_split_id()
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?,
            );
            model.next_workspace = model.next_workspace.max(
                spec.id
                    .get()
                    .checked_add(1)
                    .ok_or(Error::IdentityExhausted)?,
            );
            model.workspaces.push(Workspace {
                id: spec.id,
                group: spec.group,
                name: spec.name,
                cwd: spec.cwd,
                remote,
                panes: spec
                    .panes
                    .into_iter()
                    .map(|pane| Pane {
                        id: pane.id,
                        cwd: pane.cwd,
                        generation: 1,
                        lifecycle: Lifecycle::Starting,
                    })
                    .collect(),
                layout: spec.layout,
                active: spec.active,
            });
        }
        if model.pane_count() > limits.total_panes {
            return Err(Error::TotalPaneLimit);
        }
        if let Some(id) = active
            && model.workspace(id).is_none()
        {
            return Err(Error::UnknownWorkspace(id));
        }
        model.order_workspaces();
        model.active = active.or_else(|| model.workspaces.first().map(Workspace::id));
        model.sidebar = sidebar;
        Ok(model)
    }

    pub fn specs(&self) -> Vec<WorkspaceSpec> {
        self.workspaces
            .iter()
            .map(|workspace| WorkspaceSpec {
                id: workspace.id,
                group: workspace.group,
                name: workspace.name.clone(),
                cwd: workspace.cwd.clone(),
                remote: workspace
                    .remote
                    .as_ref()
                    .map(|remote| remote.destination().to_owned()),
                panes: workspace
                    .panes
                    .iter()
                    .map(|pane| PaneSpec {
                        id: pane.id,
                        cwd: pane.cwd.clone(),
                    })
                    .collect(),
                layout: workspace.layout.clone(),
                active: workspace.active,
            })
            .collect()
    }

    pub fn group_specs(&self) -> Vec<WorkspaceGroupSpec> {
        self.groups
            .iter()
            .map(|group| WorkspaceGroupSpec {
                id: group.id,
                name: group.name.clone(),
                collapsed: group.collapsed,
            })
            .collect()
    }

    pub(crate) fn group_mut(&mut self, id: WorkspaceGroupId) -> Result<&mut WorkspaceGroup, Error> {
        self.groups
            .iter_mut()
            .find(|group| group.id == id)
            .ok_or(Error::UnknownWorkspaceGroup(id))
    }
    /// Keep navigation indices identical to the visible folder order. Stable
    /// sorting preserves workspace order within each folder and at the root.
    pub(crate) fn order_workspaces(&mut self) {
        let groups = &self.groups;
        self.workspaces.sort_by_key(|workspace| {
            workspace
                .group
                .and_then(|id| groups.iter().position(|group| group.id == id))
                .map_or(0, |index| index + 1)
        });
    }

    pub(crate) fn workspace_mut(&mut self, id: WorkspaceId) -> Result<&mut Workspace, Error> {
        self.workspaces
            .iter_mut()
            .find(|workspace| workspace.id == id)
            .ok_or(Error::UnknownWorkspace(id))
    }
    pub(crate) fn pane_mut(&mut self, id: PaneId) -> Result<&mut Pane, Error> {
        self.workspaces
            .iter_mut()
            .flat_map(|workspace| workspace.panes.iter_mut())
            .find(|pane| pane.id == id)
            .ok_or(Error::UnknownPane(id))
    }
}
