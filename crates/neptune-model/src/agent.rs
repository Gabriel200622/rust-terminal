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

/// A pull request an agent linked to its terminal: `https://host/owner/repo/pull/N`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    url: String,
    number: u64,
}
impl PullRequest {
    /// A terminal keeps its most recent links.
    pub const MAX_PER_PANE: usize = 8;

    /// Accepts the address of a pull request or of a page under it. The saved
    /// form names only the pull request: no credentials, query or fragment.
    pub fn parse(url: &str) -> Option<Self> {
        let url = url.trim();
        if !url.get(..8)?.eq_ignore_ascii_case("https://") {
            return None;
        }
        let rest = &url[8..];
        let mut parts = rest.split(['?', '#']).next()?.split('/');
        let (host, owner, repository) = (parts.next()?, parts.next()?, parts.next()?);
        let named = |part: &str, extra: &[u8]| {
            !part.is_empty()
                && part.len() <= 100
                && !part.starts_with(['.', '-'])
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || extra.contains(&b))
        };
        if !named(host, b".-:") || !named(owner, b"-_.") || !named(repository, b"-_.") {
            return None;
        }
        if parts.next() != Some("pull") {
            return None;
        }
        let number = parts.next()?;
        if number.len() > 12 || !number.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let number = number.parse().ok().filter(|number| *number > 0)?;
        Some(Self {
            url: format!(
                "https://{}/{owner}/{repository}/pull/{number}",
                host.to_ascii_lowercase()
            ),
            number,
        })
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn number(&self) -> u64 {
        self.number
    }
    /// `owner/repo#N`.
    pub fn label(&self) -> String {
        let mut parts = self.url[8..].split('/').skip(1);
        format!(
            "{}/{}#{}",
            parts.next().unwrap_or_default(),
            parts.next().unwrap_or_default(),
            self.number
        )
    }
    /// Hosts treat owner and repository names without regard to case.
    pub fn same(&self, other: &Self) -> bool {
        self.url.eq_ignore_ascii_case(&other.url)
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
    fn pull_request_addresses_are_reduced_to_the_pull_request_or_rejected() {
        let link = PullRequest::parse(" https://GitHub.com/zevem/neptune/pull/83/files?w=1#diff ")
            .unwrap();
        assert_eq!(link.url(), "https://github.com/zevem/neptune/pull/83");
        assert_eq!(
            (link.number(), link.label()),
            (83, "zevem/neptune#83".into())
        );
        assert!(
            link.same(&PullRequest::parse("https://github.com/Zevem/Neptune/pull/83").unwrap())
        );
        for url in [
            "http://github.com/zevem/neptune/pull/83",
            "https://github.com/zevem/neptune/issues/83",
            "https://github.com/zevem/neptune/pull/0",
            "https://github.com/zevem/neptune/pull/8x",
            "https://github.com/zevem/neptune/pull/",
            "https://user@github.com/zevem/neptune/pull/83",
            "https://github.com/zevem/../pull/83",
            "https://github.com/zevem/-rf/pull/83",
            "https://github.com/pull/83",
            "file:///zevem/neptune/pull/83",
        ] {
            assert_eq!(PullRequest::parse(url), None, "{url}");
        }
    }
    #[test]
    fn linked_pull_requests_follow_the_agent_and_are_bounded_and_durable() {
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
        let link = |controller: &mut Controller, generation, number: u64| {
            controller
                .dispatch(Command::PanePullRequestLinked {
                    pane,
                    generation,
                    pull_request: PullRequest::parse(&format!(
                        "https://github.com/o/r/pull/{number}"
                    ))
                    .unwrap(),
                })
                .unwrap();
        };
        let numbers = |controller: &Controller| -> Vec<u64> {
            let pane = controller.model().pane(pane).unwrap();
            pane.pull_requests()
                .iter()
                .map(PullRequest::number)
                .collect()
        };
        // Only a terminal that runs an agent takes a link.
        link(&mut controller, generation, 1);
        assert!(numbers(&controller).is_empty());
        let open = |controller: &mut Controller, generation, agent| {
            controller
                .dispatch(Command::PaneAgentChanged {
                    pane,
                    generation,
                    agent,
                })
                .unwrap();
        };
        open(&mut controller, generation, Some(agent()));
        link(&mut controller, generation, 1);
        link(&mut controller, generation + 1, 2);
        link(&mut controller, generation, 1);
        assert_eq!(numbers(&controller), [1]);
        assert!(controller.is_dirty());
        for number in 2..=PullRequest::MAX_PER_PANE as u64 + 1 {
            link(&mut controller, generation, number);
        }
        assert_eq!(numbers(&controller), [2, 3, 4, 5, 6, 7, 8, 9]);
        // A new conversation in the same run keeps them; restore does too.
        let mut next = agent();
        next.session_id = Some("019a1234-5678-7000-8000-123456789def".into());
        open(&mut controller, generation, Some(next));
        assert_eq!(numbers(&controller).len(), 8);
        let restored = Model::restore(
            controller.model().specs(),
            Some(WorkspaceId::new(1)),
            true,
            Default::default(),
        )
        .unwrap();
        assert_eq!(restored.pane(pane).unwrap().pull_requests().len(), 8);
        open(&mut controller, generation, None);
        assert!(numbers(&controller).is_empty());
        open(&mut controller, generation, Some(agent()));
        link(&mut controller, generation, 1);
        controller.dispatch(Command::RestartPane(pane)).unwrap();
        assert!(numbers(&controller).is_empty());
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
