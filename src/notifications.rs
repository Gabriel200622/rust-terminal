//! Ephemeral, bounded attention history. Contents never enter saved state or diagnostics.
use neptune_model::{Model, PaneId};
use std::{collections::VecDeque, time::Instant};
use terminal_core::Notification;

pub const HISTORY_LIMIT: usize = 128;

pub struct Entry {
    pub sequence: u64,
    pub pane: PaneId,
    pub generation: u64,
    pub notification: Notification,
    pub unread: bool,
    pub received: Instant,
}

impl Entry {
    /// The line that names this alert. OSC 9 carries only a message.
    pub fn headline(&self) -> &str {
        if self.notification.title.is_empty() {
            &self.notification.body
        } else {
            &self.notification.title
        }
    }
}

#[derive(Default)]
pub struct Notifications {
    entries: VecDeque<Entry>,
    sequence: u64,
}

impl Notifications {
    pub fn entries(&self) -> impl DoubleEndedIterator<Item = &Entry> {
        self.entries.iter()
    }
    pub fn unread(&self, pane: Option<PaneId>) -> usize {
        self.entries
            .iter()
            .filter(|e| e.unread && pane.is_none_or(|pane| pane == e.pane))
            .count()
    }
    /// The unread count and newest unread entry among the panes `owned` accepts.
    pub fn attention(&self, owned: impl Fn(PaneId) -> bool) -> (usize, Option<&Entry>) {
        let mut unread = self.entries.iter().filter(|e| e.unread && owned(e.pane));
        let latest = unread.next_back();
        (unread.count() + usize::from(latest.is_some()), latest)
    }
    pub fn push(&mut self, pane: PaneId, generation: u64, notification: Notification) {
        if let Some(id) = &notification.id {
            self.close(pane, generation, id);
        }
        if self.entries.len() == HISTORY_LIMIT {
            self.entries.pop_front();
        }
        self.sequence += 1;
        self.entries.push_back(Entry {
            sequence: self.sequence,
            pane,
            generation,
            notification,
            unread: true,
            received: Instant::now(),
        });
    }
    pub fn close(&mut self, pane: PaneId, generation: u64, id: &str) {
        self.entries.retain(|e| {
            e.pane != pane || e.generation != generation || e.notification.id.as_deref() != Some(id)
        });
    }
    pub fn acknowledge(&mut self, pane: Option<PaneId>) {
        for entry in &mut self.entries {
            if pane.is_none_or(|pane| pane == entry.pane) {
                entry.unread = false;
            }
        }
    }
    pub fn dismiss(&mut self, sequence: u64) {
        self.entries.retain(|e| e.sequence != sequence);
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    pub fn retain_sessions(&mut self, model: &Model) {
        self.entries.retain(|e| {
            model
                .pane(e.pane)
                .is_some_and(|p| p.generation() == e.generation)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notifications_are_bounded_and_acknowledgement_is_pane_specific() {
        let mut history = Notifications::default();
        for _ in 0..200 {
            history.push(PaneId::new(1), 1, Notification::default());
        }
        history.push(PaneId::new(2), 1, Notification::default());
        assert_eq!(history.unread(None), HISTORY_LIMIT);
        history.acknowledge(Some(PaneId::new(1)));
        assert_eq!(history.unread(None), 1);
        history.retain_sessions(&Model::default());
        assert_eq!(history.entries().count(), 0);
    }
    #[test]
    fn protocol_identifiers_are_scoped_to_the_originating_session() {
        let mut history = Notifications::default();
        let message = Notification {
            id: Some("same".into()),
            title: "Hello".into(),
            ..Default::default()
        };
        history.push(PaneId::new(1), 1, message.clone());
        history.push(PaneId::new(2), 1, message.clone());
        history.push(PaneId::new(1), 1, message);
        assert_eq!(history.entries().count(), 2);
        history.close(PaneId::new(1), 2, "same");
        assert_eq!(history.entries().count(), 2);
        history.close(PaneId::new(1), 1, "same");
        assert_eq!(history.unread(Some(PaneId::new(2))), 1);
    }
    #[test]
    fn attention_reports_the_newest_unread_alert_of_the_owned_panes() {
        let mut history = Notifications::default();
        let alert = |title: &str, body: &str| Notification {
            title: title.into(),
            body: body.into(),
            ..Default::default()
        };
        history.push(PaneId::new(1), 1, alert("Build complete", "Passed"));
        history.push(PaneId::new(2), 1, alert("Other workspace", ""));
        history.push(PaneId::new(1), 1, alert("", "Needs permission"));
        let owned = |pane| pane == PaneId::new(1);
        let (unread, latest) = history.attention(owned);
        assert_eq!(unread, 2);
        assert_eq!(latest.unwrap().headline(), "Needs permission");
        history.acknowledge(Some(PaneId::new(1)));
        assert!(matches!(history.attention(owned), (0, None)));
        assert_eq!(history.attention(|_| true).0, 1);
    }
}
