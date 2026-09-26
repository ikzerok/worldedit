//! 有上限的个人阅读导航，不持有 Project 或内容副本。
use worldline_core::TargetRef;

pub const PANEL_LIMIT: usize = 2;
const HISTORY_LIMIT: usize = 64;

pub struct ReadingPanel {
    pub id: u64,
    pub target: TargetRef,
    pub history: Vec<TargetRef>,
}

#[derive(Default)]
pub struct ReadingPanels {
    panels: Vec<ReadingPanel>,
    next_id: u64,
}

impl ReadingPanels {
    pub fn pin(&mut self, target: TargetRef) -> Option<u64> {
        if self.panels.len() == PANEL_LIMIT {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.panels.push(ReadingPanel {
            id,
            target,
            history: Vec::new(),
        });
        Some(id)
    }

    pub fn get(&self, id: u64) -> Option<&ReadingPanel> {
        self.panels.iter().find(|panel| panel.id == id)
    }

    pub fn ids(&self) -> Vec<u64> {
        self.panels.iter().map(|panel| panel.id).collect()
    }

    pub fn navigate(&mut self, id: u64, target: TargetRef) {
        if let Some(panel) = self.panels.iter_mut().find(|panel| panel.id == id) {
            if panel.target != target {
                panel
                    .history
                    .push(std::mem::replace(&mut panel.target, target));
                if panel.history.len() > HISTORY_LIMIT {
                    panel.history.remove(0);
                }
            }
        }
    }

    pub fn back(&mut self, id: u64) {
        if let Some(panel) = self.panels.iter_mut().find(|panel| panel.id == id) {
            if let Some(previous) = panel.history.pop() {
                panel.target = previous;
            }
        }
    }

    pub fn close(&mut self, id: u64) {
        self.panels.retain(|panel| panel.id != id);
    }

    pub fn clear(&mut self) {
        self.panels.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_panels_keep_independent_identity_and_bounded_history() {
        let mut panels = ReadingPanels::default();
        let entity = TargetRef::new("entity", "same-id");
        let character = TargetRef::new("character", "same-id");
        let first = panels.pin(entity.clone()).unwrap();
        let second = panels.pin(character.clone()).unwrap();

        assert_ne!(first, second);
        assert!(panels.pin(TargetRef::new("world", "third")).is_none());

        panels.navigate(first, TargetRef::new("event", "one"));
        panels.navigate(first, TargetRef::new("event", "two"));
        let first_panel = panels.get(first).unwrap();
        assert_eq!(first_panel.target, TargetRef::new("event", "two"));
        assert_eq!(
            first_panel.history,
            vec![entity.clone(), TargetRef::new("event", "one")]
        );
        let second_panel = panels.get(second).unwrap();
        assert_eq!(second_panel.target, character);
        assert!(second_panel.history.is_empty());

        panels.back(first);
        assert_eq!(
            panels.get(first).unwrap().target,
            TargetRef::new("event", "one")
        );
        panels.back(first);
        assert_eq!(panels.get(first).unwrap().target, entity);
        assert_eq!(panels.get(second).unwrap().target.kind, "character");

        for index in 0..=HISTORY_LIMIT {
            panels.navigate(first, TargetRef::new("event", &format!("next-{index}")));
        }
        assert_eq!(panels.get(first).unwrap().history.len(), HISTORY_LIMIT);
        panels.clear();
        let replacement = panels.pin(TargetRef::new("entity", "replacement")).unwrap();
        assert!(replacement > second, "window identity must not be reused");
        assert!(panels.get(first).is_none());
    }
}
