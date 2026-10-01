use pace_model::{
    Axis, Layout, Limits, Model, PaneId, PaneSpec, SplitId, WorkspaceId, WorkspaceSpec,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::Write,
    path::{Path, PathBuf},
};

pub const SCHEMA_VERSION: u32 = 1;
const MAX_STATE_BYTES: u64 = 8 * 1024 * 1024;

/// This DTO is the disk contract. Runtime layout serialization cannot change it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateSnapshot {
    pub version: u32,
    pub workspaces: Vec<SavedWorkspace>,
    pub active: Option<WorkspaceId>,
    pub sidebar: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedWorkspace {
    pub id: WorkspaceId,
    pub name: String,
    pub cwd: PathBuf,
    pub panes: Vec<SavedPane>,
    pub layout: SavedLayout,
    pub active: PaneId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedPane {
    pub id: PaneId,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SavedLayout {
    Pane {
        pane: PaneId,
    },
    Split {
        id: SplitId,
        axis: SavedAxis,
        ratio: f32,
        first: Box<SavedLayout>,
        second: Box<SavedLayout>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedAxis {
    Vertical,
    Horizontal,
}

impl StateSnapshot {
    pub fn from_model(model: &Model) -> Self {
        Self {
            version: SCHEMA_VERSION,
            workspaces: model
                .workspaces()
                .iter()
                .map(|workspace| SavedWorkspace {
                    id: workspace.id(),
                    name: workspace.name().into(),
                    cwd: workspace.cwd().into(),
                    panes: workspace
                        .panes()
                        .iter()
                        .map(|pane| SavedPane {
                            id: pane.id(),
                            cwd: pane.cwd().into(),
                        })
                        .collect(),
                    layout: SavedLayout::from_layout(workspace.layout()),
                    active: workspace.active(),
                })
                .collect(),
            active: model.active_workspace(),
            sidebar: model.sidebar(),
        }
    }

    /// Conversion enforces the same invariants as new commands without checking
    /// directories; callers can use it for headless snapshots and round trips.
    pub fn into_model(self, limits: Limits) -> Result<Model, pace_model::Error> {
        Model::restore(
            self.workspaces
                .into_iter()
                .map(SavedWorkspace::into_spec)
                .collect(),
            self.active,
            self.sidebar,
            limits,
        )
    }
}

impl SavedWorkspace {
    fn into_spec(self) -> WorkspaceSpec {
        WorkspaceSpec {
            id: self.id,
            name: self.name,
            cwd: self.cwd,
            panes: self
                .panes
                .into_iter()
                .map(|pane| PaneSpec {
                    id: pane.id,
                    cwd: pane.cwd,
                })
                .collect(),
            layout: self.layout.into_layout(),
            active: self.active,
        }
    }
}

impl SavedLayout {
    fn from_layout(layout: &Layout) -> Self {
        match layout {
            Layout::Leaf(pane) => Self::Pane { pane: *pane },
            Layout::Split {
                id,
                axis,
                ratio,
                first,
                second,
            } => Self::Split {
                id: *id,
                axis: match axis {
                    Axis::Vertical => SavedAxis::Vertical,
                    Axis::Horizontal => SavedAxis::Horizontal,
                },
                ratio: *ratio,
                first: Box::new(Self::from_layout(first)),
                second: Box::new(Self::from_layout(second)),
            },
        }
    }
    fn into_layout(self) -> Layout {
        match self {
            Self::Pane { pane } => Layout::Leaf(pane),
            Self::Split {
                id,
                axis,
                ratio,
                first,
                second,
            } => Layout::Split {
                id,
                axis: match axis {
                    SavedAxis::Vertical => Axis::Vertical,
                    SavedAxis::Horizontal => Axis::Horizontal,
                },
                ratio,
                first: Box::new(first.into_layout()),
                second: Box::new(second.into_layout()),
            },
        }
    }
}

#[derive(Debug)]
pub struct LoadReport {
    pub model: Option<Model>,
    pub diagnostics: Vec<String>,
    /// False for unreadable state, unsupported formats or failed recovery copy.
    /// The desktop may continue with defaults, but must not save over this file.
    pub can_write: bool,
    pub migrated: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LegacyState {
    workspaces: Vec<LegacyWorkspace>,
    active: usize,
    sidebar: bool,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyWorkspace {
    name: String,
    cwd: PathBuf,
    #[serde(default)]
    layout: Option<LegacyLayout>,
    #[serde(default)]
    panes: Vec<LegacyPane>,
    #[serde(default)]
    active: u64,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyPane {
    id: u64,
    cwd: PathBuf,
}
#[derive(Debug, Deserialize)]
enum LegacyLayout {
    Leaf(u64),
    Split {
        vertical: bool,
        ratio: f32,
        first: Box<LegacyLayout>,
        second: Box<LegacyLayout>,
    },
}

pub fn load_state(path: &Path, limits: Limits) -> LoadReport {
    let mut report = LoadReport {
        model: None,
        diagnostics: Vec::new(),
        can_write: true,
        migrated: false,
    };
    let bytes = match read_state(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return report,
        Err(error) => {
            report.can_write = false;
            report.diagnostics.push(format!(
                "Cannot read workspace state {}: {error}; saving this state file is disabled",
                path.display()
            ));
            return report;
        }
    };
    let value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(error) => {
            report.diagnostics.push(format!(
                "Invalid workspace state {}: {error}",
                path.display()
            ));
            preserve_original(path, &bytes, &mut report);
            return report;
        }
    };
    if let Some(version) = value.get("version") {
        let Some(version_number) = version.as_u64() else {
            report.diagnostics.push(format!(
                "Invalid workspace schema version {version} in {}",
                path.display()
            ));
            preserve_original(path, &bytes, &mut report);
            return report;
        };
        if version_number != u64::from(SCHEMA_VERSION) {
            report.can_write = false;
            report.diagnostics.push(format!("Unsupported workspace schema version {version} in {}; preserving original and disabling state saves", path.display()));
            return report;
        }
        match serde_json::from_value::<StateSnapshot>(value) {
            Ok(snapshot) => restore_versioned(snapshot, limits, &mut report),
            Err(error) => report.diagnostics.push(format!(
                "Invalid workspace schema in {}: {error}",
                path.display()
            )),
        }
    } else {
        match serde_json::from_value::<LegacyState>(value) {
            Ok(legacy) => {
                report.migrated = true;
                restore_legacy(legacy, limits, &mut report);
            }
            Err(error) => report.diagnostics.push(format!(
                "Invalid legacy workspace state in {}: {error}",
                path.display()
            )),
        }
    }
    // Migration itself preserves semantics. Invalid/missing entries or a broken
    // schema require a recovery copy before any startup save can replace them.
    if !report.diagnostics.is_empty() {
        preserve_original(path, &bytes, &mut report);
    }
    report
}

fn read_state(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_STATE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_STATE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "workspace state exceeds the 8 MiB restoration limit",
        ));
    }
    Ok(bytes)
}

fn restore_versioned(snapshot: StateSnapshot, limits: Limits, report: &mut LoadReport) {
    let requested_active = snapshot.active;
    let mut specs = Vec::new();
    let mut total = 0;
    for mut workspace in snapshot.workspaces {
        if !workspace.cwd.is_dir() {
            report.diagnostics.push(format!(
                "Skipped workspace {:?}: directory {} does not exist",
                workspace.name,
                workspace.cwd.display()
            ));
            continue;
        }
        if specs.len() >= limits.workspaces || total + workspace.panes.len() > limits.total_panes {
            report.diagnostics.push(format!(
                "Skipped workspace {:?}: configured resource limit exceeded",
                workspace.name
            ));
            continue;
        }
        for pane in &mut workspace.panes {
            if !pane.cwd.is_dir() {
                report.diagnostics.push(format!(
                    "Pane {} in {:?}: missing directory {}; using {}",
                    pane.id,
                    workspace.name,
                    pane.cwd.display(),
                    workspace.cwd.display()
                ));
                pane.cwd.clone_from(&workspace.cwd);
            }
        }
        let name = workspace.name.clone();
        let spec = workspace.into_spec();
        let mut candidate = specs.clone();
        candidate.push(spec.clone());
        match Model::restore(candidate, None, snapshot.sidebar, limits) {
            Ok(_) => {
                total += spec.panes.len();
                specs.push(spec);
            }
            Err(error) => report
                .diagnostics
                .push(format!("Skipped invalid workspace {name:?}: {error}")),
        }
    }
    let active = requested_active.filter(|id| specs.iter().any(|workspace| workspace.id == *id));
    if requested_active.is_some() && active.is_none() {
        report.diagnostics.push(
            "Restored active workspace was unavailable; selected the first remaining workspace"
                .into(),
        );
    }
    match Model::restore(specs, active, snapshot.sidebar, limits) {
        Ok(model) => report.model = Some(model),
        Err(error) => report
            .diagnostics
            .push(format!("Workspace state validation failed: {error}")),
    }
}

fn restore_legacy(legacy: LegacyState, limits: Limits, report: &mut LoadReport) {
    let mut specs = Vec::new();
    let mut next_pane = 1u64;
    let mut next_split = 1u64;
    let mut active = None;
    let mut total = 0;
    for (position, workspace) in legacy.workspaces.into_iter().enumerate() {
        if !workspace.cwd.is_dir() {
            report.diagnostics.push(format!(
                "Skipped workspace {:?}: directory {} does not exist",
                workspace.name,
                workspace.cwd.display()
            ));
            continue;
        }
        if specs.len() >= limits.workspaces || total >= limits.total_panes {
            report.diagnostics.push(format!(
                "Skipped workspace {:?}: configured resource limit exceeded",
                workspace.name
            ));
            continue;
        }
        if workspace.name.trim().is_empty() {
            report
                .diagnostics
                .push("Skipped legacy workspace with an empty name".into());
            continue;
        }
        let id = WorkspaceId::new(position as u64 + 1);
        let mut mapping = HashMap::new();
        let mut panes = Vec::new();
        for pane in workspace.panes {
            if panes.len() >= limits.panes_per_workspace
                || total + panes.len() >= limits.total_panes
            {
                report.diagnostics.push(format!(
                    "Workspace {:?}: omitted panes beyond configured resource limit",
                    workspace.name
                ));
                break;
            }
            if mapping.contains_key(&pane.id) {
                report.diagnostics.push(format!(
                    "Workspace {:?}: omitted duplicate legacy pane {}",
                    workspace.name, pane.id
                ));
                continue;
            }
            let pane_id = PaneId::new(next_pane);
            next_pane += 1;
            mapping.insert(pane.id, pane_id);
            let cwd = if pane.cwd.is_dir() {
                pane.cwd
            } else {
                report.diagnostics.push(format!(
                    "Pane {} in {:?}: missing directory {}; using {}",
                    pane.id,
                    workspace.name,
                    pane.cwd.display(),
                    workspace.cwd.display()
                ));
                workspace.cwd.clone()
            };
            panes.push(PaneSpec { id: pane_id, cwd });
        }
        if panes.is_empty() {
            if limits.panes_per_workspace == 0 {
                report.diagnostics.push(format!(
                    "Skipped workspace {:?}: pane limit is zero",
                    workspace.name
                ));
                continue;
            }
            panes.push(PaneSpec {
                id: PaneId::new(next_pane),
                cwd: workspace.cwd.clone(),
            });
            next_pane += 1;
        }
        let mut layout = Layout::Leaf(panes[0].id);
        for pane in panes.iter().skip(1) {
            layout = Layout::Split {
                id: SplitId::new(next_split),
                axis: Axis::Vertical,
                ratio: 0.5,
                first: Box::new(layout),
                second: Box::new(Layout::Leaf(pane.id)),
            };
            next_split += 1;
        }
        if let Some(saved_layout) = workspace.layout {
            let mut seen = HashSet::new();
            match migrate_layout(saved_layout, &mapping, &mut seen, &mut next_split) {
                Some(restored) if seen.len() == panes.len() => layout = restored,
                _ => report.diagnostics.push(format!(
                    "Workspace {:?}: repaired invalid legacy split layout",
                    workspace.name
                )),
            }
        }
        let selected = mapping
            .get(&workspace.active)
            .copied()
            .unwrap_or(panes[0].id);
        total += panes.len();
        if position == legacy.active {
            active = Some(id);
        }
        specs.push(WorkspaceSpec {
            id,
            name: workspace.name,
            cwd: workspace.cwd,
            panes,
            layout,
            active: selected,
        });
    }
    match Model::restore(specs, active, legacy.sidebar, limits) {
        Ok(model) => report.model = Some(model),
        Err(error) => report
            .diagnostics
            .push(format!("Legacy workspace validation failed: {error}")),
    }
}

fn migrate_layout(
    layout: LegacyLayout,
    mapping: &HashMap<u64, PaneId>,
    seen: &mut HashSet<PaneId>,
    next_split: &mut u64,
) -> Option<Layout> {
    match layout {
        LegacyLayout::Leaf(old) => {
            let pane = *mapping.get(&old)?;
            seen.insert(pane).then_some(Layout::Leaf(pane))
        }
        LegacyLayout::Split {
            vertical,
            ratio,
            first,
            second,
        } => {
            if !ratio.is_finite() || !(0.1..=0.9).contains(&ratio) {
                return None;
            }
            let id = SplitId::new(*next_split);
            *next_split += 1;
            Some(Layout::Split {
                id,
                axis: if vertical {
                    Axis::Vertical
                } else {
                    Axis::Horizontal
                },
                ratio,
                first: Box::new(migrate_layout(*first, mapping, seen, next_split)?),
                second: Box::new(migrate_layout(*second, mapping, seen, next_split)?),
            })
        }
    }
}

fn preserve_original(path: &Path, bytes: &[u8], report: &mut LoadReport) {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let result = (|| -> std::io::Result<PathBuf> {
        let mut backup = tempfile::Builder::new()
            .prefix("pace-workspaces-recovery-")
            .suffix(".json")
            .tempfile_in(parent)?;
        backup.write_all(bytes)?;
        backup.as_file().sync_all()?;
        let (_file, backup_path) = backup.keep().map_err(|error| error.error)?;
        Ok(backup_path)
    })();
    match result {
        Ok(backup) => report.diagnostics.push(format!(
            "Original workspace state preserved at {}",
            backup.display()
        )),
        Err(error) => {
            report.can_write = false;
            report.diagnostics.push(format!(
                "Could not preserve {}: {error}; state saving is disabled",
                path.display()
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pace_model::{Command, Controller};

    fn fixture(directory: &Path) -> Vec<u8> {
        include_str!("fixtures/workspaces-unversioned.json")
            .replace(
                "@DIRECTORY@",
                &directory.to_string_lossy().replace('\\', "\\\\"),
            )
            .into_bytes()
    }
    fn sample(directory: &Path) -> StateSnapshot {
        let mut controller = Controller::new(Model::default());
        controller
            .dispatch(Command::AddWorkspace {
                cwd: directory.into(),
                name: "fixture".into(),
            })
            .unwrap();
        let workspace = controller.model().active_workspace().unwrap();
        let pane = controller.model().active_pane().unwrap();
        controller
            .dispatch(Command::SplitPane {
                workspace,
                pane,
                axis: Axis::Vertical,
                cwd: directory.into(),
            })
            .unwrap();
        StateSnapshot::from_model(controller.model())
    }

    #[test]
    fn current_unversioned_fixture_migrates_without_losing_layout_or_selection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspaces.json");
        std::fs::write(&path, fixture(directory.path())).unwrap();
        let report = load_state(&path, Limits::default());
        assert!(report.migrated);
        assert!(report.can_write);
        assert!(report.diagnostics.is_empty());
        let model = report.model.unwrap();
        let workspace = &model.workspaces()[0];
        assert_eq!(workspace.panes().len(), 2);
        assert_eq!(workspace.active(), workspace.panes()[1].id());
        assert!(
            matches!(workspace.layout(), Layout::Split { axis: Axis::Vertical, ratio, .. } if *ratio == 0.65)
        );
    }

    #[test]
    fn independent_versioned_dto_round_trip_retains_split_identities() {
        let directory = tempfile::tempdir().unwrap();
        let snapshot = sample(directory.path());
        let bytes = serde_json::to_vec(&snapshot).unwrap();
        let parsed: StateSnapshot = serde_json::from_slice(&bytes).unwrap();
        let model = parsed.into_model(Limits::default()).unwrap();
        assert_eq!(
            serde_json::to_value(StateSnapshot::from_model(&model)).unwrap(),
            serde_json::to_value(snapshot).unwrap()
        );
    }

    #[test]
    fn corrupt_state_is_preserved_before_overwrite_is_allowed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspaces.json");
        let original = b"{broken state";
        std::fs::write(&path, original).unwrap();
        let report = load_state(&path, Limits::default());
        assert!(report.can_write);
        assert!(report.model.is_none());
        let backup = std::fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|entry| *entry != path)
            .unwrap();
        assert_eq!(std::fs::read(backup).unwrap(), original);
        assert_eq!(std::fs::read(path).unwrap(), original);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|message| message.contains("preserved"))
        );
    }

    #[test]
    fn unsupported_future_schema_cannot_be_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspaces.json");
        let original = br#"{"version":99,"future_data":{"keep":"everything"}}"#;
        std::fs::write(&path, original).unwrap();
        let report = load_state(&path, Limits::default());
        assert!(!report.can_write);
        assert!(report.model.is_none());
        assert!(report.diagnostics[0].contains("Unsupported"));
        assert_eq!(std::fs::read(path).unwrap(), original);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn malformed_version_is_corrupt_and_archived_instead_of_reported_as_future() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspaces.json");
        std::fs::write(&path, br#"{"version":"typo"}"#).unwrap();
        let report = load_state(&path, Limits::default());
        assert!(report.can_write);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|message| message.contains("Invalid workspace schema version"))
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[test]
    fn oversized_state_is_read_only_and_never_allocated_or_overwritten_in_full() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspaces.json");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_STATE_BYTES + 1).unwrap();
        let report = load_state(&path, Limits::default());
        assert!(!report.can_write);
        assert!(report.diagnostics[0].contains("8 MiB"));
        assert_eq!(std::fs::metadata(path).unwrap().len(), MAX_STATE_BYTES + 1);
    }

    #[test]
    fn unreadable_state_path_disables_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let report = load_state(directory.path(), Limits::default());
        assert!(!report.can_write);
        assert!(report.diagnostics[0].contains("Cannot read"));
    }

    #[test]
    fn missing_pane_directory_falls_back_and_archives_original_with_context() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspaces.json");
        let mut snapshot = sample(directory.path());
        snapshot.workspaces[0].panes[1].cwd = directory.path().join("missing");
        let bytes = serde_json::to_vec(&snapshot).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let report = load_state(&path, Limits::default());
        assert!(report.can_write);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|message| message.contains("Pane 2") && message.contains("missing"))
        );
        assert_eq!(
            report.model.unwrap().workspaces()[0].panes()[1].cwd(),
            directory.path()
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[test]
    fn restore_skips_missing_workspace_and_selects_valid_successor() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspaces.json");
        let mut snapshot = sample(directory.path());
        let mut second = snapshot.workspaces[0].clone();
        second.id = WorkspaceId::new(2);
        second.name = "valid successor".into();
        second.panes = vec![SavedPane {
            id: PaneId::new(3),
            cwd: directory.path().into(),
        }];
        second.layout = SavedLayout::Pane {
            pane: PaneId::new(3),
        };
        second.active = PaneId::new(3);
        snapshot.workspaces[0].cwd = directory.path().join("missing");
        snapshot.workspaces.push(second);
        std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        let report = load_state(&path, Limits::default());
        let model = report.model.unwrap();
        assert_eq!(model.active_workspace(), Some(WorkspaceId::new(2)));
        assert_eq!(model.workspaces().len(), 1);
    }

    #[test]
    fn invalid_new_schema_workspace_is_skipped_without_constructing_invalid_model() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspaces.json");
        let mut snapshot = sample(directory.path());
        snapshot.workspaces[0].active = PaneId::new(999);
        std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        let report = load_state(&path, Limits::default());
        assert!(report.model.unwrap().workspaces().is_empty());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|message| message.contains("invalid workspace"))
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[test]
    fn legacy_duplicate_layout_is_repaired_and_archived() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspaces.json");
        let text = String::from_utf8(fixture(directory.path()))
            .unwrap()
            .replace("\"Leaf\": 12", "\"Leaf\": 9");
        std::fs::write(&path, text).unwrap();
        let report = load_state(&path, Limits::default());
        let model = report.model.unwrap();
        assert_eq!(model.workspaces()[0].layout().leaves().len(), 2);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|message| message.contains("repaired invalid legacy"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn failed_recovery_copy_keeps_corrupt_file_read_only() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspaces.json");
        let original = b"broken";
        std::fs::write(&path, original).unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
        let report = load_state(&path, Limits::default());
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        // Privileged test environments can bypass directory permissions; the
        // injected no-write path is meaningful only when backup creation fails.
        if report
            .diagnostics
            .iter()
            .any(|message| message.contains("Could not preserve"))
        {
            assert!(!report.can_write);
        }
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}
