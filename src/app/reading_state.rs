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
