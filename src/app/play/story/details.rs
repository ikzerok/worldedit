//! 原有调试、路径、状态和锚点内容共用；紧凑布局只改变容器。
use super::{Requests, WorldeditApp};
use crate::app::play::{debugger, evidence_navigation::EvidenceNavigationAccess};
use crate::theme;

impl WorldeditApp {
    pub(super) fn play_debugger_details(
        &mut self,
        ui: &mut egui::Ui,
        requests: &mut Requests,
        evidence_access: &EvidenceNavigationAccess,
    ) {
        let cur_version = self.version;
        let can_replay = self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.result.has_errors());
        let Some(play) = &mut self.play else { return };
        ui.separator();
        debugger::render_debugger_controls(
            ui,
            &mut self.replay_debugger,
            play,
            cur_version,
            can_replay,
            debugger::DebuggerRequests {
                replay: &mut requests.replay,
                failure: &mut requests.failure,
                evidence: &mut requests.evidence,
                keyboard: &mut self.play_keyboard,
            },
            evidence_access,
        );
        ui.separator();
        let Some(story) = &play.story else {
            return;
        };
        ui.label(format!(
            "回合 {} · 故事线 {}",
            story.turns(),
            story.storyline()
        ));
        if let Some(node) = story.current_node() {
            ui.label(format!("节点 {node}"));
        }
        let met = story.met_list();
        ui.label(format!(
            "在场:{}",
            if met.is_empty() {
                "无".into()
            } else {
                met.join(", ")
            }
        ));
        if !story.states().is_empty() {
            ui.separator();
            egui::CollapsingHeader::new(format!("状态变更记录 · {}", story.state_history().len()))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("runtime-state-history")
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for record in story.state_history().iter().rev() {
                                ui.label(format!(
                                    "{}：{} → {}",
                                    record.state,
                                    record.before.join(", "),
                                    record.after.join(", ")
                                ));
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} · 轮次 {}",
                                        record.node.as_deref().unwrap_or(""),
                                        record.turn
                                    ))
                                    .small()
                                    .color(theme::MUTED()),
                                );
                                if let Some(note) = &record.note {
                                    ui.label(note);
                                }
                            }
                        });
                });
        }
        // 锚点记录(按发生序)
        ui.separator();
        ui.heading("锚点记录");
        let anchors: Vec<worldline_runtime::AnchorRecord> = story.anchors().to_vec();
        if anchors.is_empty() {
            ui.colored_label(
                theme::MUTED(),
                "(暂无;记录由 anchor 语句与漂流、叙事身份、人物变动产生)",
            );
        }
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                for a in anchors.iter().rev() {
                    let mut line = format!("◆ [{}] {}", a.kind.label(), a.name);
                    if let Some(d) = &a.detail {
                        line.push_str(&format!(" → {d}"));
                    }
                    ui.colored_label(theme::ANCHOR(), line);
                    if let Some(n) = &a.note {
                        ui.indent("anchor-note", |ui| {
                            ui.colored_label(theme::MUTED(), format!("↳ {n}"));
                        });
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "   @{} · 故事线 {} · 回合 {}",
                            a.node.as_deref().unwrap_or("?"),
                            a.storyline,
                            a.turn
                        ))
                        .size(10.0)
                        .color(theme::MUTED()),
                    );
                }
            });
        ui.separator();
    }
}
