use super::*;
use crate::{
    draft_rehearsal_worker::{Action, View},
    theme,
};
use worldline_core::draft_rehearsal::{DraftRehearsalScope, DraftRehearsalSourceKind};
mod inspector;

impl WorldeditApp {
    pub(in crate::app) fn draft_rehearsal_dialog(&mut self, ctx: &egui::Context) {
        self.draft_rehearsal.preparation_blocks_frame(ctx);
        self.poll_draft_rehearsal(ctx);
        let Some(pending) = &self.draft_rehearsal.pending else {
            return;
        };
        let mut cancel = false;
        let mut confirm = false;
        let mut retry = false;
        let busy_ime = self.rehearsal_composing();
        let screen = ctx.screen_rect();
        egui::Modal::new(egui::Id::new("draft-rehearsal-scope")).show(ctx, |ui| {
            ui.set_width((screen.width() - 40.0).clamp(180.0, 680.0));
            ui.heading("试演当前正文草稿");
            // 主要动作固定在材料之上；窄窗/高缩放下长范围只滚动材料，不能推走取消。
            ui.horizontal_wrapped(|ui| {
                confirm |= theme::add_enabled(
                    ui,
                    pending.view.is_some() && pending.error.is_none() && !busy_ime,
                    theme::primary("明确开始这份草稿试演"),
                )
                .clicked();
                cancel |=
                    theme::add_enabled(ui, !busy_ime, egui::Button::new("取消准备")).clicked();
                ui.label("seed");
                ui.add(
                    egui::DragValue::new(&mut self.draft_rehearsal.seed)
                        .range(0..=9_007_199_254_740_991u64),
                );
            });
            let content_height =
                (screen.height() - ui.min_rect().height() - 48.0).clamp(24.0, 420.0);
            egui::ScrollArea::vertical()
                .id_salt("draft-rehearsal-confirm-scroll")
                .max_height(content_height)
                .show(ui, |ui| {
                    ui.label("只为本次试演使用已应用工程 + 列出的未应用正文；作者输入保持原处");
                    if let Some(error) = &pending.error {
                        ui.colored_label(theme::ERROR(), error);
                        retry |=
                            theme::add_enabled(ui, !busy_ime, egui::Button::new("重新核对当前稿"))
                                .clicked();
                    } else if let Some(view) = &pending.view {
                        render_scope(ui, &view.scope);
                    } else {
                        ui.spinner();
                        ui.label(if pending.worker.is_none() {
                            "等待上一份隔离编译退出；不会叠加后台编译副本"
                        } else {
                            "正在后台核对完整来源并编译；可以取消，草稿完整保留"
                        });
                        #[cfg(not(target_arch = "wasm32"))]
                        ui.label(theme::muted(
                            "原生编译内部暂不能抢占；取消后丢弃结果，完成后释放资源",
                        ));
                    }
                    ui.label(theme::muted(
                        "本模式不保存正式路径。先返回并明确应用正文，再开始普通试玩录制",
                    ));
                });
        });
        if cancel {
            self.draft_rehearsal.pending = None;
        } else if retry {
            self.draft_rehearsal.pending = None;
            self.request_draft_rehearsal(ctx);
        } else if confirm {
            self.confirm_draft_rehearsal(ctx);
        }
    }

    pub(in crate::app::play) fn draft_rehearsal_tab(&mut self, ctx: &egui::Context) {
        self.poll_draft_rehearsal(ctx);
        let mut return_origin = false;
        let mut restart = false;
        let mut regular = false;
        let mut close = false;
        let mut action = None;
        let mut localization_navigation = None;
        let locale_name = self
            .draft_rehearsal
            .running
            .as_ref()
            .and_then(|r| r.view.presentation.as_ref())
            .map(|i| i.request.target_locale.clone())
            .unwrap_or_default();
        let busy_ime = self.rehearsal_composing();
        let pending = self.draft_rehearsal.pending.is_some();
        // 使用完整应用 chrome 消耗后的空间；屏幕尺寸不能代表正文的可用视口。
        let available = ctx.available_rect();
        let compact = available.width() < 720.0 || available.height() < 360.0;
        egui::TopBottomPanel::top("draft-rehearsal-actions").show(ctx, |ui| {
            if compact {
                // 两行固定入口；详情和长提示在下面的同一滚动区，不挤走正文。
                ui.horizontal(|ui| {
                    ui.strong("草稿隔离试演");
                    return_origin |= ui.button("返回原草稿位置").clicked();
                    ui.menu_button("试演操作", |ui| {
                        restart |= ui.button("重新试演当前稿…").clicked();
                        regular |= ui.button("保留试演，查看已应用稿").clicked();
                        close |= ui.button("关闭试演").clicked();
                        if restart || regular || close {
                            ui.close();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("draft-rehearsal-pane")
                        .selected_text(pane_label(&self.draft_rehearsal.pane))
                        .width(150.0)
                        .show_ui(ui, |ui| pane_choices(ui, &mut self.draft_rehearsal.pane));
                    if let Some(running) = &self.draft_rehearsal.running {
                        ui.label(if running.stale && running.stopped {
                            "旧快照 · 已停止"
                        } else if running.stale {
                            "旧快照"
                        } else if running.stopped {
                            "已停止"
                        } else if running.worker.as_ref().is_some_and(SessionWorker::busy) {
                            "执行中"
                        } else if running.paused {
                            "已暂停"
                        } else if running.view.ended {
                            "已结束"
                        } else {
                            "独立快照"
                        });
                    }
                });
            } else {
                ui.horizontal_wrapped(|ui| {
                    ui.heading("当前正文草稿 · 隔离试演");
                    return_origin |= ui.button("返回原草稿位置").clicked();
                    restart |= ui.button("重新试演当前稿…").clicked();
                    regular |= ui.button("保留试演，查看已应用稿").clicked();
                    close |= ui.button("关闭试演").clicked();
                });
                if let Some(running) = &self.draft_rehearsal.running {
                    session_context(
                        ui,
                        running,
                        self.draft_rehearsal.seed,
                        self.draft_rehearsal.notice.as_deref(),
                    );
                }
                ui.horizontal_wrapped(|ui| pane_choices(ui, &mut self.draft_rehearsal.pane));
            }
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(running) = &mut self.draft_rehearsal.running else {
                ui.label("没有当前草稿试演"); return;
            };
            let busy = running.worker.as_ref().is_some_and(SessionWorker::busy);
            let enabled = !busy && !running.stale && !running.stopped && !busy_ime && !pending;
            egui::ScrollArea::vertical().id_salt("draft-rehearsal-body")
                .auto_shrink([false, false]).show(ui, |ui| {
                if compact {
                    session_context(ui, running, self.draft_rehearsal.seed,
                        self.draft_rehearsal.notice.as_deref());
                }
                if busy { ui.spinner(); ui.label("后台执行中；只推进本次独立快照"); }
                if let Some(error) = &running.view.error { ui.colored_label(theme::ERROR(), error); }
                match self.draft_rehearsal.pane {
                    Pane::Story => {
                        ui.horizontal_wrapped(|ui| {
                            if running.view.ended { ui.label("这条路径已正常结束"); }
                            if running.stopped { ui.label("已停止；仅保留已收到的实际证据"); }
                            else if running.paused {
                                if theme::add_enabled(ui, enabled, egui::Button::new("继续此快照")).clicked() {
                                    running.paused = false;
                                    if running.view.choices.is_empty() && !running.view.ended {
                                        action = Some(Action::Continue { budget: self.draft_rehearsal.budget });
                                    }
                                }
                            } else if theme::add_enabled(ui, !busy && !pending, egui::Button::new("暂停选择")).clicked() {
                                running.paused = true;
                            }
                            if theme::add_enabled(ui, !running.stopped, egui::Button::new("停止并保留已收到的证据")).clicked() {
                                running.worker = None; running.stopped = true; running.paused = true;
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.label("单次步数"); ui.add(egui::DragValue::new(&mut self.draft_rehearsal.budget.max_steps).range(0..=100_000));
                            ui.label("毫秒"); ui.add(egui::DragValue::new(&mut self.draft_rehearsal.budget.time_budget_ms).range(0..=250));
                        });
                        if let Some(outcome) = running.view.outcome { ui.label(outcome.message()); }
                        ui.label(theme::muted("真实文本累计1MiB/32768项；状态与证据有独立预算；不自动重播已执行语句"));
                        ui.separator();
                        crate::app::play::localization::identity_label(ui, running.view.presentation.as_ref(), &mut self.replay_debugger.locale.parallel);
                        if running.view.presentation.is_some() {
                            localization_navigation = crate::app::play::localization::outputs(ui, &running.localized_outputs, self.replay_debugger.locale.parallel);
                        } else { ui.add(egui::Label::new(&running.transcript).wrap()); }
                        ui.separator();
                        let stamp = running.view.inspection.as_ref().map(|page| page.stamp);
                        for choice in &running.view.choices {
                            if choice.localization.is_some() {
                                if let Some(request) = crate::app::play::localization::item(ui, &choice.label, &choice.links, choice.localization.as_ref(), self.replay_debugger.locale.parallel) { localization_navigation = Some(request); }
                            }
                            // 新运行或新选择观测使用新控件身份，旧Enter按住/重复不能接管下一组。
                            let response = ui.push_id(
                                ("draft-rehearsal-choice", stamp.map(|stamp| (stamp.run_id, stamp.revision)), &choice.id),
                                |ui| theme::add_enabled(ui, enabled && !running.paused && choice.enabled,
                                    egui::Button::new(format!("选择：{}", choice.label)).wrap()),
                            ).inner;
                            if response.clicked() {
                                ui.memory_mut(|memory| memory.surrender_focus(response.id));
                                action = Some(Action::Choose { id: choice.id.clone(), budget: self.draft_rehearsal.budget });
                            }
                            if let Some(reason) = &choice.disabled_reason { ui.label(theme::muted(reason)); }
                        }
                        if running.view.choices.is_empty() && !running.view.ended && !running.paused
                            && theme::add_enabled(ui, enabled, egui::Button::new("推进此快照")).clicked()
                        { action = Some(Action::Continue { budget: self.draft_rehearsal.budget }); }
                    }
                    Pane::Scope => render_scope(ui, &running.view.scope),
                    Pane::State => inspector::render(ui, running, !busy && !pending && running.worker.is_some(), enabled, busy_ime, &mut action),
                    Pane::Conditions => render_conditions(ui, &running.view, enabled, &mut action),
                }
            });
        });
        if let Some(navigation) = localization_navigation {
            self.open_localized_item(ctx, navigation, &locale_name, true);
        }
        if close {
            self.draft_rehearsal.running = None;
            self.draft_rehearsal.active = false;
        } else if regular {
            self.draft_rehearsal.active = false;
        } else if return_origin {
            self.return_rehearsal_origin(ctx);
        } else if restart {
            self.request_draft_rehearsal(ctx);
        } else if let Some(action) = action {
            self.send_rehearsal_action(action);
        }
    }
}

fn pane_label(pane: &Pane) -> &'static str {
    match pane {
        Pane::Story => "正文与选择",
        Pane::State => "状态与变化",
        Pane::Conditions => "实际条件证据",
        Pane::Scope => "输入范围",
    }
}

fn pane_choices(ui: &mut egui::Ui, pane: &mut Pane) {
    for choice in [Pane::Story, Pane::State, Pane::Conditions, Pane::Scope] {
        let label = pane_label(&choice);
        ui.selectable_value(pane, choice, label);
    }
}

fn session_context(ui: &mut egui::Ui, running: &Running, seed: u64, notice: Option<&str>) {
    if running.stale {
        ui.colored_label(
            theme::WARNING(),
            "这份试演快照已过期：保留旧输出与实际状态；继续选择和回源需重新试演",
        );
    }
    ui.label(format!(
        "seed {} · 回合 {} · {}",
        running.view.seed.unwrap_or(seed),
        running.view.turns,
        running.view.node.as_deref().unwrap_or("无当前节点")
    ));
    if let Some(notice) = notice {
        ui.colored_label(theme::WARNING(), notice);
    }
}

fn render_scope(ui: &mut egui::Ui, scope: &DraftRehearsalScope) {
    ui.label(format!(
        "入口 {} · 语言 {} · fingerprint {}",
        scope.entry.display(),
        scope.language_version,
        scope.runtime_fingerprint
    ));
    for source in &scope.sources {
        let label = match source.kind {
            DraftRehearsalSourceKind::Applied => "已应用源",
            DraftRehearsalSourceKind::WritingDraft => "纳入未应用正文",
        };
        ui.label(format!(
            "{label} · {} · {} 字节{}",
            source.path.display(),
            source.bytes,
            source
                .generation
                .map(|n| format!(" · 代次 {n}"))
                .unwrap_or_default()
        ));
    }
    ui.separator();
    ui.label(format!("排除 {} 项未提交输入", scope.excluded_inputs.len()));
    for excluded in &scope.excluded_inputs {
        ui.label(format!("未纳入：{} · {}", excluded.kind, excluded.source));
    }
}

fn render_conditions(ui: &mut egui::Ui, view: &View, enabled: bool, action: &mut Option<Action>) {
    let access = super::super::evidence_navigation::EvidenceNavigationAccess::rehearsal(
        &view.conditions,
        (!enabled).then(|| "旧快照、输入组合、已停止或请求尚未返回；仅保留只读证据".into()),
    );
    let mut jump = None;
    super::super::evidence::render_rehearsal(ui, &view.conditions, &access, &mut jump);
    if let Some(jump) = jump {
        *action = Some(Action::EvidenceSource {
            source: jump.source,
        });
    }
}
