//! 只展示已存在的真实证据，不从终值diff推断写入。
use super::super::evidence_navigation::{EvidenceNavigationAccess, EvidenceNavigationRequest};
use worldline_runtime::OwnedStory;
pub(super) fn render(
    ui: &mut egui::Ui,
    story: &OwnedStory,
    run_version: u64,
    edit_version: u64,
    access: &EvidenceNavigationAccess,
    jump: &mut Option<EvidenceNavigationRequest>,
    comparison: &mut bool,
) {
    ui.label("值变化不是写入历史；同值写入也可能发生。此处不推测最后写入");
    *comparison |= ui
        .button("打开路线对照：查看已录制路径的动作 / 变量写入证据")
        .clicked();
    ui.label("路线证据需使用已有路径并显式运行对照；当前检查不会自动重放");
    egui::ScrollArea::vertical().id_salt("inspection-real-evidence").max_height(400.0).show(ui,|ui| {
        super::super::evidence::render(ui,story.choice_evidence().unwrap_or_default(),run_version,edit_version,access,jump);
        ui.separator();ui.heading("本次运行的实际状态变更记录");
        let records=story.state_history();
        if records.is_empty(){ui.label("当前保存状态没有状态动作记录；这不提供全局变量写入历史");}
        for record in records.iter().rev().take(64) {
            ui.label(format!("{} · {} → {}",record.state,display_set(&record.before),display_set(&record.after)));
            ui.label(format!("回合 {} · {} · {:?}",record.turn,record.node.as_deref().unwrap_or("未记录节点"),record.kind));
        }
        if records.len()>64 {ui.label(format!("本处仅显示最近64 / {}条；原始状态历史仍保留",records.len()));}
        ui.label("这些记录没有精确写入来源；不能由节点或终值猜一个源码行。检查点恢复的记录也可能来自本次记录起点之前");
    });
}
fn display_set(tags: &[String]) -> String {
    let shown = tags
        .iter()
        .take(12)
        .map(|tag| {
            let value = tag.chars().take(80).collect::<String>();
            if value.len() < tag.len() {
                format!("{value}…")
            } else {
                value
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    if tags.len() > 12 {
        format!("[{shown}, …]（共{}项，显示省略）", tags.len())
    } else {
        format!("[{shown}]")
    }
}
