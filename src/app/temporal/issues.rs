//! 时间问题与比较：只展示当前core快照的环、受阻关联和follows证据。
use crate::{app::WorldeditApp, theme};
use worldline_core::timeline::{TemporalComparisonReason, TemporalEdge, TemporalRelation};
#[derive(Default)]
pub(in crate::app) struct TemporalIssues {
    pub open: bool,
    pub left: String,
    pub right: String,
    pub focus: bool,
    pub return_focus: Option<egui::Id>,
    pub entry_focus: Option<egui::Id>,
}
pub(super) fn relation_label(relation: TemporalRelation) -> &'static str {
    match relation {
        TemporalRelation::Before => "左侧事件先于右侧事件",
        TemporalRelation::After => "左侧事件后于右侧事件",
        TemporalRelation::SameEvent => "同一事件",
        TemporalRelation::UnorderedSameRoot => "同根无先后约束（不表示同时）",
        TemporalRelation::DifferentRoots => "独立时间根，不能比较先后",
        TemporalRelation::Invalid => "当前时间线无效，不能作先后结论",
        TemporalRelation::Unknown => "事件或时间信息未知",
    }
}
fn reason_label(reason: TemporalComparisonReason) -> &'static str {
    match reason {
        TemporalComparisonReason::PartialTimeline => "当前编译不完整，请先修复时间问题或其他错误",
        TemporalComparisonReason::UnknownEvent => "稳定事件 ID 不存在，请重新选择；不会按同名替换",
        TemporalComparisonReason::MissingTime => "事件没有可用的时间归属",
        TemporalComparisonReason::InvalidTimeRoot => "时间根无效，请定位时间归属",
        TemporalComparisonReason::DifferentOrderScopes => "当前语言版本中的比较范围不同",
    }
}
impl WorldeditApp {
    pub(in crate::app) fn open_temporal_issues(&mut self, ctx: &egui::Context) {
        self.sync_edit_layers(ctx);
        if !self.temporal_issues.open {
            self.temporal_issues.return_focus = ctx.memory(|m| m.focused());
        }
        self.temporal_issues.open = true;
        self.temporal_issues.focus = true;
        if self.temporal_issues.left.is_empty() {
            self.temporal_issues.left = self
                .event_editor
                .as_ref()
                .and_then(|e| e.original.clone())
                .unwrap_or_default();
        }
    }
    pub(in crate::app) fn temporal_issues_window(&mut self, ctx: &egui::Context) {
        if !self.temporal_issues.open {
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let timeline = snapshot.result.analysis.timeline.clone();
        let catalog = snapshot.result.analysis.catalog.clone();
        let display = |id: &str| {
            catalog
                .object(&worldline_core::TargetRef::new("event", id))
                .map(|o| o.display.clone())
                .unwrap_or_else(|| id.to_owned())
        };
        let mut open = true;
        let mut close = false;
        let mut source = None;
        let viewport = ctx.screen_rect().shrink(16.0);
        egui::Window::new("时间问题与先后比较")
            .id(egui::Id::new("temporal-issues"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(700.0)
            .default_height((viewport.height() - 100.0).max(250.0))
            .max_width((viewport.width() - 30.0).max(280.0))
            .max_height((viewport.height() - 100.0).max(250.0))
            .constrain_to(viewport)
            .show(ctx, |ui| {
                ui.label(theme::muted(format!(
                    "当前编译快照 · {} 个真正时间环 · {} 个受阻下游",
                    timeline.cycles.len(),
                    timeline.blocked.len()
                )));
                ui.label(theme::muted(
                    "按名称选择两个事件 · 只采用显式 follows，不以层级推断同时",
                ));
                for (label, salt, left) in [
                    ("左侧事件", "temporal-left-picker", true),
                    ("右侧事件", "temporal-right-picker", false),
                ] {
                    let current = if left {
                        &self.temporal_issues.left
                    } else {
                        &self.temporal_issues.right
                    };
                    let mut target = (!current.is_empty())
                        .then(|| worldline_core::TargetRef::new("event", current));
                    let (changed, button_id) = crate::app::object_picker::object_picker_focused(
                        ui,
                        salt,
                        label,
                        &mut target,
                        &catalog,
                        &["event"],
                        left && self.temporal_issues.focus,
                    );
                    if changed {
                        let id = target.map(|t| t.id).unwrap_or_default();
                        if left {
                            self.temporal_issues.left = id;
                        } else {
                            self.temporal_issues.right = id;
                        }
                    }
                    if left {
                        self.temporal_issues.entry_focus = Some(button_id);
                        if self.temporal_issues.focus {
                            self.temporal_issues.focus = false;
                        }
                    }
                }
                ui.collapsing("按稳定 ID 直接输入（可选）", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.temporal_issues.left)
                            .id(egui::Id::new("temporal-compare-left-input"))
                            .hint_text("左侧事件稳定 ID")
                            .desired_width(f32::INFINITY),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.temporal_issues.right)
                            .id(egui::Id::new("temporal-compare-right-input"))
                            .hint_text("右侧事件稳定 ID")
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.separator();
                // 头部会因可选ID、失效提示和选择器展开改变高度；按真实剩余空间留固定footer。
                let body_height = (ui.available_height() - 82.0)
                    .min(viewport.bottom() - ui.cursor().top() - 82.0)
                    .max(40.0);
                egui::ScrollArea::vertical()
                    .id_salt("temporal-issues-content")
                    .max_height(body_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if !self.temporal_issues.left.trim().is_empty()
                            && !self.temporal_issues.right.trim().is_empty()
                        {
                            let comparison = timeline.compare(
                                self.temporal_issues.left.trim(),
                                self.temporal_issues.right.trim(),
                            );
                            ui.strong(relation_label(comparison.relation));
                            ui.label(format!(
                                "{} ↔ {}",
                                display(&comparison.left),
                                display(&comparison.right)
                            ));
                            if let Some(reason) = comparison.reason {
                                ui.label(reason_label(reason));
                            }
                            if !comparison.evidence.is_empty() {
                                ui.label("先后依据 · 以下边按真实先于方向排列");
                                draw_edges(
                                    ui,
                                    &comparison.evidence,
                                    &display,
                                    &mut source,
                                    &self.project.root,
                                );
                            }
                            ui.separator();
                        }
                        if timeline.cycles.is_empty() {
                            ui.label("没有检测到时间环；其他编译错误仍可能使时间比较无效");
                        }
                        for cycle in &timeline.cycles {
                            theme::card().show(ui, |ui| {
                                ui.colored_label(
                                    theme::ERROR(),
                                    format!("时间环 · {} 位成员", cycle.members.len()),
                                );
                                for id in &cycle.members {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(format!("环成员 · {}", display(id)));
                                        if let Some(object) = catalog
                                            .object(&worldline_core::TargetRef::new("event", id))
                                        {
                                            if ui
                                                .small_button("定位事件")
                                                .on_hover_text(format!(
                                                    "{}:{} · {id}",
                                                    object.file, object.line
                                                ))
                                                .clicked()
                                            {
                                                source = Some((object.file.clone(), object.line));
                                            }
                                        }
                                    });
                                }
                                let blocked = timeline
                                    .blocked
                                    .iter()
                                    .filter(|b| b.cycle_ids.contains(&cycle.id))
                                    .collect::<Vec<_>>();
                                ui.colored_label(
                                    theme::WARNING(),
                                    format!("受此环阻断 · {} 个下游（不是环成员）", blocked.len()),
                                );
                                for item in blocked {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(display(&item.event));
                                        if let Some(object) = catalog.object(
                                            &worldline_core::TargetRef::new("event", &item.event),
                                        ) {
                                            if ui
                                                .small_button("定位受阻事件")
                                                .on_hover_text(format!(
                                                    "{}:{} · {}",
                                                    object.file, object.line, item.event
                                                ))
                                                .clicked()
                                            {
                                                source = Some((object.file.clone(), object.line));
                                            }
                                        }
                                        if item.cycle_ids.len() > 1 {
                                            ui.label(theme::muted(format!(
                                                "关联 {} 个环",
                                                item.cycle_ids.len()
                                            )));
                                        }
                                    });
                                }
                                egui::CollapsingHeader::new("闭环证据 · 每一条都来自真实 follows")
                                    .id_salt((&cycle.id, "witness"))
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        draw_edges(
                                            ui,
                                            &cycle.witness,
                                            &display,
                                            &mut source,
                                            &self.project.root,
                                        )
                                    });
                                ui.collapsing("技术详情", |ui| {
                                    ui.label(format!("A213 · 环标识 {} · 稳定最短闭环", cycle.id));
                                });
                            });
                        }
                    });
                ui.separator();
                close = ui.button("关闭并返回原焦点 · Esc").clicked();
                ui.label(theme::muted(
                    "修正源码后自动使用新快照；本面板不删除约束、不应用草稿",
                ));
            });
        self.temporal_issues.open = open && !close && source.is_none();
        if let Some((file, line)) = source {
            self.jump_to_file(&file, line, 1);
        } else if !self.temporal_issues.open {
            if let Some(id) = self.temporal_issues.return_focus {
                ctx.memory_mut(|m| m.request_focus(id));
            }
        }
    }
}
fn draw_edges(
    ui: &mut egui::Ui,
    edges: &[TemporalEdge],
    display: &impl Fn(&str) -> String,
    source: &mut Option<(String, u32)>,
    root: &std::path::Path,
) {
    for (index, edge) in edges.iter().enumerate() {
        ui.push_id(("edge", index), |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "{} → {}",
                    display(&edge.before),
                    display(&edge.after)
                ));
                if ui.small_button("定位 follows").clicked() {
                    *source = Some((edge.file.clone(), edge.line));
                }
            });
            let line = format!(
                "{}:{} · 行级来源",
                theme::relative_source(root, std::path::Path::new(&edge.file)),
                edge.line
            );
            ui.add(egui::Label::new(theme::muted(&line)).truncate())
                .on_hover_text(format!(
                    "{}:{}\n{} → {}",
                    edge.file, edge.line, edge.before, edge.after
                ));
        });
    }
}
