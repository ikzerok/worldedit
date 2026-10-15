//! 本帧全局操作预算：常用动作优先，溢出进入有名字的菜单。
use crate::chrome::action_layout::{button_width, row_width};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ActionLayout {
    Full,
    Folded,
    Stacked,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ChromeLayoutBudget {
    available: f32,
    full: f32,
    folded: f32,
}

impl ChromeLayoutBudget {
    pub(super) fn measure(ui: &egui::Ui, navigation: bool, state: &str) -> Self {
        let gap = ui.spacing().item_spacing.x;
        let navigation = if navigation {
            button_width(ui, "导航") + gap
        } else {
            0.0
        };
        let common = navigation
            + row_width(
                &[button_width(ui, "保存全部"), button_width(ui, state)],
                gap,
            );
        let full = common
            + gap
            + row_width(
                &[
                    "▶ 试玩",
                    "搜索",
                    "导出与发布",
                    "工程",
                    "编辑",
                    "视图",
                    "外观",
                ]
                .map(|label| button_width(ui, label)),
                gap,
            );
        let folded = common + gap + button_width(ui, "更多");
        Self {
            available: (ui.available_width() - gap).max(0.0),
            full,
            folded,
        }
    }

    pub(super) fn choose(self, focus: bool) -> ActionLayout {
        if !focus && self.full <= self.available {
            ActionLayout::Full
        } else if self.folded <= self.available {
            ActionLayout::Folded
        } else {
            ActionLayout::Stacked
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_keeps_both_states_and_focus_never_expands_low_frequency_groups() {
        let budget = ChromeLayoutBudget {
            available: 900.0,
            full: 850.0,
            folded: 400.0,
        };
        assert_eq!(budget.choose(false), ActionLayout::Full);
        assert_eq!(budget.choose(true), ActionLayout::Folded);
        assert_eq!(
            ChromeLayoutBudget {
                available: 450.0,
                ..budget
            }
            .choose(false),
            ActionLayout::Folded
        );
        assert_eq!(
            ChromeLayoutBudget {
                available: 350.0,
                ..budget
            }
            .choose(false),
            ActionLayout::Stacked
        );
    }
}
