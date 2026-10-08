//! 当前真实试玩的只读检查区；类型、比较与分页均来自runtime。
mod evidence;
mod navigation;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
use super::super::WorldeditApp;
use crate::theme;
use worldline_runtime::*;

#[derive(Default)]
pub(in crate::app) struct InspectionUi {
    pub open: bool,
    pub query: StateInspectionQuery,
    pub page: Option<StateInspectionPage>,
    pub error: Option<String>,
    pub selected: Option<InspectionKey>,
    cache_query: Option<StateInspectionQuery>,
    focus_search: bool,
    show_evidence: bool,
    return_focus: Option<egui::Id>,
}
pub(super) struct SourceRequest {
    pub stamp: InspectionStamp,
    pub key: InspectionKey,
    pub source: worldline_core::state_inspection_source::DeclarationSource,
}
impl InspectionUi {
    pub(super) fn show(&mut self, ctx: &egui::Context) {
        self.return_focus = ctx.memory(|memory| memory.focused());
        self.open = true;
        self.focus_search = true;
    }
    fn refresh(&mut self, story: &OwnedStory) {
        let stamp = story.inspection_stamp();
        if self.page.as_ref().is_some_and(|page| {
            page.stamp.run_id != stamp.run_id
                || page.stamp.trace_generation != stamp.trace_generation
        }) {
            self.selected = None;
            self.query.offset = 0;
        }
        if self.page.as_ref().is_some_and(|page| page.stamp != stamp) {
            self.query.offset = 0;
            self.query.expected_stamp = None;
        }
        if self.page.as_ref().is_some_and(|page| page.stamp == stamp)
            && self.cache_query.as_ref() == Some(&self.query)
        {
            return;
        }
        match story.inspect_state(&self.query) {
            Ok(page) => {
                self.page = Some(page);
                self.error = None;
            }
            Err(error) => {
                self.page = None;
                self.error = Some(error.message);
            }
        }
        self.cache_query = Some(self.query.clone());
    }
}
impl WorldeditApp {
    pub(super) fn render_state_inspection(&mut self, ctx: &egui::Context) {
        if !self.replay_debugger.inspection.open {
            return;
        }
        let evidence_access = self.evidence_navigation_access();
        let source_reason = self
            .play
            .as_ref()
            .and_then(|play| self.play_source_guard(&play.scope).err());
        let Some(play) = &self.play else {
            self.replay_debugger.inspection = Default::default();
            return;
        };
        let Some(story) = &play.story else {
            return;
        };
        let inspector = &mut self.replay_debugger.inspection;
        inspector.refresh(story);
        let mut open = true;
        let mut close = false;
        let mut comparison = false;
        let mut evidence_jump = None;
        let mut source_request = None;
        let size = ctx.screen_rect().size();
        egui::Window::new("状态检查 · 当前真实试玩")
            .id(egui::Id::new("live-state-inspection"))
            .open(&mut open)
            .resizable(true)
            .vscroll(true)
            .default_size(egui::vec2(
                (size.x - 40.0).min(860.0),
                (size.y - 100.0).min(640.0),
            ))
            .max_size(egui::vec2(
                (size.x - 20.0).max(200.0),
                (size.y - 50.0).max(200.0),
            ))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    close |= ui.button("返回试玩与选择").clicked();
                    ui.selectable_value(&mut inspector.show_evidence, false, "状态与变化");
                    ui.selectable_value(&mut inspector.show_evidence, true, "实际条件 / 动作证据");
                    ui.label(theme::muted("只读 · 查看期间保留当前位置"));
                });
                if play.version != self.version {
                    ui.colored_label(
                        theme::WARNING(),
                        "当前仍是旧稿运行快照；重新开始才运行新稿，旧来源不可跳转",
                    );
                }
                if let Some(error) = &play.error {
                    ui.colored_label(theme::ERROR(), format!("运行错误：{error}"));
                }
                if play.stopped {
                    ui.label("试玩已停止，保留最后实际状态");
                } else if play.paused {
                    ui.label("试玩已暂停，检查不会恢复推进");
                }
                if !story.is_ended() {
                    ui.label("当前路径尚未到结尾；局部结果不证明所有分支");
                }
                if inspector.show_evidence {
                    evidence::render(
                        ui,
                        story,
                        play.version,
                        self.version,
                        &evidence_access,
                        &mut evidence_jump,
                        &mut comparison,
                    );
                    return;
                }
                let before = inspector.query.clone();
                let search = ui.add(
                    egui::TextEdit::singleline(&mut inspector.query.text)
                        .hint_text("查名称、片段或当前值…")
                        .desired_width(f32::INFINITY)
                        .char_limit(256),
                );
                if inspector.focus_search {
                    search.request_focus();
                    inspector.focus_search = false;
                }
                ui.horizontal_wrapped(|ui| {
                    for (group, label) in [
                        (None, "全部"),
                        (Some(InspectionGroup::Global), "全局"),
                        (Some(InspectionGroup::Local), "调用局部"),
                        (Some(InspectionGroup::State), "状态集"),
                    ] {
                        ui.selectable_value(&mut inspector.query.group, group, label);
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut inspector.query.changed_only, "只看变化");
                    ui.label("比较基线");
                    ui.selectable_value(
                        &mut inspector.query.compare_to,
                        InspectionBaseline::Previous,
                        "上一观测",
                    );
                    ui.selectable_value(
                        &mut inspector.query.compare_to,
                        InspectionBaseline::First,
                        "首次观测",
                    );
                });
                if inspector.query != before {
                    inspector.query.offset = 0;
                    inspector.query.expected_stamp = None;
                    inspector.refresh(story);
                }
                if let Some(error) = &inspector.error {
                    ui.colored_label(theme::ERROR(), error);
                }
                let Some(page) = &inspector.page else {
                    return;
                };
                ui.label(format!(
                    "{} · 运行 {} · fingerprint {}",
                    status(page.status),
                    page.stamp.run_id,
                    page.stamp.fingerprint
                ));
                ui.label(observation_note(page));
                if page.history_omitted {
                    ui.colored_label(
                        theme::WARNING(),
                        "部分历史值已省略；不能完整检索或比较这些历史项。当前值仍完整检索，原路径记录不受影响",
                    );
                }
                ui.label(format!(
                    "匹配 {} / 共 {} 项 · 不可比较 {} 项",
                    page.total_matches, page.total_items, page.incomparable_items
                ));
                ui.horizontal(|ui| {
                    if theme::add_enabled(ui, page.offset > 0, egui::Button::new("上一页"))
                        .clicked()
                    {
                        inspector.query.offset = page.offset.saturating_sub(page.limit);
                        inspector.query.expected_stamp = Some(page.stamp);
                    }
                    ui.label(format!(
                        "{}–{}",
                        if page.items.is_empty() {
                            0
                        } else {
                            page.offset + 1
                        },
                        page.offset + page.items.len()
                    ));
                    if theme::add_enabled(
                        ui,
                        page.next_offset.is_some(),
                        egui::Button::new("下一页"),
                    )
                    .clicked()
                    {
                        inspector.query.offset = page.next_offset.unwrap_or(0);
                        inspector.query.expected_stamp = Some(page.stamp);
                    }
                });
                ui.separator();
                // 窗口统一滚动；嵌套同向滚动会让窄窗内的值和来源按钮互相截获滚轮。
                ui.push_id("live-state-inspection-rows", |ui| {
                    if page.items.is_empty() {
                        ui.label("此筛选无匹配项；不代表路径或其他状态没有问题");
                    }
                    for item in &page.items {
                        ui.push_id(&item.key, |ui| {
                            ui.group(|ui| {
                                let title = format!(
                                    "{} · {}{}",
                                    group(item.key.group),
                                    item.key.name,
                                    item.fragment
                                        .as_ref()
                                        .map(|name| format!(
                                            " · {name} 调用 #{}",
                                            item.key.call_id.unwrap_or(0)
                                        ))
                                        .unwrap_or_default()
                                );
                                if ui
                                    .selectable_label(
                                        inspector.selected.as_ref() == Some(&item.key),
                                        title,
                                    )
                                    .clicked()
                                {
                                    inspector.selected = Some(item.key.clone());
                                }
                                let change = match inspector.query.compare_to {
                                    InspectionBaseline::First => item.first_change,
                                    InspectionBaseline::Previous => item.previous_change,
                                };
                                ui.label(change_label(change));
                                if ui.available_width() >= 620.0 {
                                    ui.columns(3, |columns| {
                                        cell(&mut columns[0], "首次", &item.first);
                                        cell(&mut columns[1], "上次", &item.previous);
                                        cell(&mut columns[2], "当前", &item.current);
                                    });
                                } else {
                                    cell(ui, "当前", &item.current);
                                    cell(ui, "上次", &item.previous);
                                    cell(ui, "首次", &item.first);
                                }
                                let reason = source_reason.as_deref().or_else(|| {
                                    item.source.is_none().then_some("局部值暂无可验证声明来源")
                                });
                                let label = if item.key.group == InspectionGroup::State {
                                    "定位状态声明"
                                } else {
                                    "定位变量声明"
                                };
                                let response = theme::add_enabled(
                                    ui,
                                    reason.is_none(),
                                    egui::Button::new(label),
                                );
                                if response.clicked() {
                                    source_request =
                                        item.source.clone().map(|source| SourceRequest {
                                            stamp: page.stamp,
                                            key: item.key.clone(),
                                            source,
                                        });
                                }
                                if let Some(reason) = reason {
                                    response.on_hover_text(reason);
                                }
                            });
                        });
                    }
                });
                ui.label(theme::muted(
                    "值差异不说明写入因果；同值写入也可能发生。声明位置不是最后写入位置",
                ));
            });
        if !open || close || comparison {
            inspector.open = false;
            if let Some(id) = inspector.return_focus.take() {
                ctx.memory_mut(|memory| memory.request_focus(id));
            }
            self.play_keyboard.restore_after_inspection(ctx);
        }
        if comparison {
            self.comparison.active = true;
        }
        if let Some(request) = evidence_jump {
            self.jump_to_evidence_source(ctx, &request);
        }
        if let Some(request) = source_request {
            self.jump_to_inspection_source(ctx, &request);
        }
    }
}
fn cell(ui: &mut egui::Ui, label: &str, cell: &InspectionCell) {
    ui.label(theme::muted(label));
    ui.monospace(&cell.display);
    if let Some(value) = &cell.value {
        ui.label(theme::muted(value.kind_label()));
    }
    if cell.truncated {
        ui.label("显示已截断；完整值未提供");
    }
}
fn group(group: InspectionGroup) -> &'static str {
    match group {
        InspectionGroup::Global => "全局变量",
        InspectionGroup::Local => "调用局部",
        InspectionGroup::State => "状态标签集",
    }
}
fn change_label(change: InspectionChange) -> &'static str {
    match change {
        InspectionChange::Changed => "有变化",
        InspectionChange::Unchanged => "值未变化",
        InspectionChange::NotComparable => "不可比较（无基线 / 不同调用 / 已省略）",
    }
}
fn status(status: InspectionStatus) -> &'static str {
    match status {
        InspectionStatus::Ready => "尚未完成首次观测",
        InspectionStatus::Choice => "等待选择",
        InspectionStatus::Ended => "故事已结束",
        InspectionStatus::Advancing => "已选待续行 / 部分推进",
        InspectionStatus::StepBudgetExceeded => "步数预算暂停",
        InspectionStatus::TimeBudgetExceeded => "时间预算暂停",
        InspectionStatus::Cancelled => "推进已取消",
        InspectionStatus::Failed => "运行错误的部分状态",
    }
}
fn observation_note(page: &StateInspectionPage) -> String {
    let first = page
        .first_observation
        .map_or("尚无记录".into(), |n| format!("观测 #{n}"));
    let previous = page
        .previous_observation
        .map_or("不存在，不能补零".into(), |n| format!("观测 #{n}"));
    let current = page.current_observation.map_or(
        "当前为实时部分状态；上次指最近完成的真实观测".into(),
        |n| format!("当前为观测 #{n}；上次指其前一真实观测"),
    );
    format!("首次：{first}；上次：{previous}。{current}")
}
