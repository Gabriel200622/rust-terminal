//! Resume references only; prompts, commands and process memory are never saved.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Claude,
    Codex,
}
impl AgentKind {
    pub fn executable(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSession {
    pub kind: AgentKind,
    /// None means the CLI was open but did not report a resumable session.
    pub session_id: Option<String>,
    pub cwd: PathBuf,
}
impl AgentSession {
    pub fn is_valid(&self) -> bool {
        self.cwd.is_absolute()
            && self.cwd.as_os_str().len() <= 32768
            && self.session_id.as_ref().is_none_or(|id| {
                // Both providers use UUIDs. Do not accept options, names or commands.
                id.len() == 36
                    && id.bytes().enumerate().all(|(i, b)| {
                        if matches!(i, 8 | 13 | 18 | 23) {
                            b == b'-'
                        } else {
                            b.is_ascii_hexdigit()
                        }
                    })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Controller, Model, Remote, WorkspaceId};
    fn agent() -> AgentSession {
        AgentSession {
            kind: AgentKind::Codex,
            session_id: Some("019a1234-5678-7000-8000-123456789abc".into()),
            cwd: std::env::temp_dir(),
        }
    }
    #[test]
    fn resume_references_reject_options_commands_and_relative_paths() {
        let mut value = agent();
        assert!(value.is_valid());
        for id in [
            "--last",
            "abc; touch x",
            "latest",
            "",
            "019a1234-5678-7000-8000-123456789abz",
        ] {
            value.session_id = Some(id.into());
            assert!(!value.is_valid());
        }
        value.session_id = None;
        value.cwd = "relative".into();
        assert!(!value.is_valid());
    }
    #[test]
    fn agent_metadata_is_targeted_durable_and_cleared_by_reverse_actions() {
        let mut controller = Controller::new(Model::default());
        controller
            .dispatch(Command::AddWorkspace {
                cwd: std::env::temp_dir(),
                name: "a".into(),
                remote: None,
                group: None,
            })
            .unwrap();
        let pane = controller.model().active_pane().unwrap();
        let generation = controller.model().pane(pane).unwrap().generation();
        controller
            .dispatch(Command::PaneAgentChanged {
                pane,
                generation,
                agent: Some(agent()),
            })
            .unwrap();
        let saved = controller.model().specs();
        let restored =
            Model::restore(saved, Some(WorkspaceId::new(1)), true, Default::default()).unwrap();
        assert_eq!(restored.pane(pane).unwrap().agent(), Some(&agent()));
        controller.dispatch(Command::RestartPane(pane)).unwrap();
        assert!(controller.model().pane(pane).unwrap().agent().is_none());
        controller
            .dispatch(Command::PaneAgentChanged {
                pane,
                generation,
                agent: Some(agent()),
            })
            .unwrap();
        assert!(controller.model().pane(pane).unwrap().agent().is_none());
        let generation = generation + 1;
        controller
            .dispatch(Command::PaneAgentChanged {
                pane,
                generation,
                agent: Some(agent()),
            })
            .unwrap();
        controller
            .dispatch(Command::SetWorkspaceRemote {
                workspace: WorkspaceId::new(1),
                remote: Some(Remote::parse("host").unwrap().destination().into()),
            })
            .unwrap();
        assert!(controller.model().pane(pane).unwrap().agent().is_none());
        controller
            .dispatch(Command::PaneAgentChanged {
                pane,
                generation: generation + 1,
                agent: Some(agent()),
            })
            .unwrap();
        assert!(controller.model().pane(pane).unwrap().agent().is_none());
        controller.dispatch(Command::ClosePane(pane)).unwrap();
        controller
            .dispatch(Command::PaneAgentChanged {
                pane,
                generation,
                agent: Some(agent()),
            })
            .unwrap();
        assert!(controller.model().pane(pane).is_none());
    }
}
