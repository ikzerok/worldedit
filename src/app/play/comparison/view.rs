//! 主内容对照；首屏身份、有效性与首差异不依赖技术抽屉。
use super::{
    navigation::{source_button, ComparisonSourceRequest, NavigationAccess},
    ComparedRoutes,
};
use crate::{app::WorldeditApp, theme};
use worldline_runtime::{RouteSideResult, RouteStatus};

pub(super) fn status_text(side: &RouteSideResult) -> &'static str {
    match &side.status {
        RouteStatus::Replayed if side.complete && side.ended => "✓ 完整结束并验证通过",
        RouteStatus::Replayed => "◐ 已记录区段通过 · 非完整结局",
        RouteStatus::Diverged => "! 与原路径分歧 · 实际停止状态",
        RouteStatus::StepBudgetExceeded => "◐ 步数预算耗尽 · 部分结果",
        RouteStatus::TimeBudgetExceeded => "◐ 时间预算耗尽 · 部分结果",
        RouteStatus::Cancelled => "■ 已取消 · 非完整结果",
        RouteStatus::IncompleteTrace => "◐ 记录不完整 · 实际停止状态",
        RouteStatus::StoryFailed => "! 运行失败 · 实际停止状态",
        RouteStatus::OutputBudgetExceeded => "◐ 输出预算耗尽 · 部分结果",
    }
}

fn route_header(
    ui: &mut egui::Ui,
    letter: &str,
    name: &str,
    side: Option<&RouteSideResult>,
    running: bool,
) {
    egui::Frame::group(ui.style())
        .fill(theme::CARD())
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.add(
                egui::Label::new(egui::RichText::new(format!("{letter}  {name}")).strong())
                    .truncate(),
            )
            .on_hover_text(name);
            if let Some(side) = side {
                ui.label(status_text(side));
                ui.label(theme::muted(format!(
                    "{} · seed {}",
                    if side.origin.kind == "checkpoint" {
                        "检查点起点"
                    } else {
                        "故事入口起点"
                    },
                    side.origin.seed
                )));
                if side.states.is_none() || side.vars.is_none() {
                    ui.colored_label(theme::WARNING(), "未取得实际状态");
                }
            } else {
                ui.label(if running {
                    "正在验证共同已应用稿…"
                } else {
                    "尚未验证"
                });
            }
        });
}

fn first_difference(
    ui: &mut egui::Ui,
    compared: &ComparedRoutes,
    access: &NavigationAccess<'_>,
    request: &mut Option<ComparisonSourceRequest>,
) {
    ui.strong("首个不同选择");
    let alignment = &compared.result.alignment;
    if !alignment.comparable {
        ui.colored_label(
            theme::WARNING(),
            alignment
                .reason
                .as_deref()
                .unwrap_or("起点不可对齐；只能并列查看各自实际结果"),
        );
        return;
    }
    if let Some(difference) = &alignment.first_difference {
        ui.label(theme::muted(format!(
            "已验证共同选择前缀 {} · 第 {} 次选择不同",
            alignment.common_prefix,
            difference.index + 1
        )));
        ui.columns(2, |columns| {
            for (display, ui) in columns.iter_mut().enumerate() {
                let actual = compared.display_index(display == 1);
                let input = if actual == 0 {
                    &difference.left
                } else {
                    &difference.right
                };
                ui.push_id(("first-choice", actual), |ui| {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.add(
                            egui::Label::new(format!(
                                "{}  {}",
                                if display == 0 { "A" } else { "B" },
                                input.choice.label
                            ))
                            .truncate(),
                        )
                        .on_hover_text(&input.choice.label);
                        source_button(
                            ui,
                            compared,
                            egui::Id::new(("choice", actual, difference.index)),
                            input.source.as_ref(),
                            access,
                            "打开选择声明",
                            request,
                        );
                    });
                });
            }
        });
    } else {
        ui.label(format!(
            "已验证共同前缀 {} 次选择；当前可比较区段未出现不同选择。",
            alignment.common_prefix
        ));
        ui.label(theme::muted(
            "没有首差异不表示路线相同；一侧可能尚未验证后续选择。",
        ));
    }
}

impl WorldeditApp {
    pub(in crate::app::play) fn comparison_tab(&mut self, ctx: &egui::Context) {
        let source_current =
            self.comparison.result.as_ref().is_some_and(|compared| {
                compared.scope.matches_project(&self.project, self.version)
            });
        let blocked = self
            .comparison
            .result
            .as_ref()
            .and_then(|result| self.play_source_guard(&result.scope).err());
        let restoring = self.comparison.restore_scroll || self.comparison.restore_focus.is_some();
        let ime = source_current
            && (self.ime_composing || self.command_palette.ime || self.command_palette.ime_frame);
        let context = egui::Id::new((
            "comparison-focus-context",
            &self.project.root,
            self.version,
            self.comparison.result.as_ref().map(|r| (r.id, r.reversed)),
            self.comparison.a,
            self.comparison.b,
            &self.comparison.selected_state,
            self.comparison.selected_action,
            &self.comparison.selected_variable,
            self.comparison.selected_write,
        ));
        let access = NavigationAccess {
            blocked: blocked.as_deref(),
            focus: super::focus::FocusReveal::for_frame(ctx, restoring),
            ime_focus: super::focus::ImeFocus::prepare(
                ctx,
                context,
                ime,
                blocked.is_some(),
                restoring,
            ),
        };
        let mut request = None;
        let mut target = None;
        let mut run = false;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("路线对照");
            ui.label(theme::muted(
                "从两条真实路径查看差异；显式验证同一份当前已应用稿",
            ));
            let running = self.comparison.job.is_some();
            if self.comparison.a.is_none() && !self.replay_debugger.saved_paths.is_empty() {
                self.comparison.a = Some(0);
            }
            if self.comparison.b.is_none() && self.replay_debugger.saved_paths.len() > 1 {
                self.comparison.b = Some(1);
            }
            crate::theme::add_enabled_ui(ui, !running, |ui| {
                ui.columns(2, |columns| {
                    for (i, ui) in columns.iter_mut().enumerate() {
                        let selected = if i == 0 {
                            &mut self.comparison.a
                        } else {
                            &mut self.comparison.b
                        };
                        let label = selected
                            .and_then(|n| self.replay_debugger.saved_paths.get(n))
                            .map(|p| p.name.as_str())
                            .unwrap_or("选择实际路径");
                        egui::ComboBox::from_id_salt(("comparison-path", i))
                            .width(ui.available_width().max(100.0))
                            .truncate()
                            .selected_text(format!("{}  {label}", if i == 0 { "A" } else { "B" }))
                            .show_ui(ui, |ui| {
                                for (index, path) in
                                    self.replay_debugger.saved_paths.iter().enumerate()
                                {
                                    ui.selectable_value(selected, Some(index), &path.name);
                                }
                            });
                    }
                });
            });
            ui.horizontal_wrapped(|ui| {
                if crate::theme::add_enabled(ui, !running, egui::Button::new("交换 A/B")).clicked()
                {
                    self.comparison.swap();
                }
                run = crate::theme::add_enabled(
                    ui,
                    !running
                        && self
                            .comparison
                            .has_paths(self.replay_debugger.saved_paths.len()),
                    theme::primary("比较当前已应用稿"),
                )
                .clicked();
                if let Some(job) = &self.comparison.job {
                    if ui.button("取消比较").clicked() {
                        job.cancellation.cancel();
                    }
                }
                ui.label(format!("当前已应用稿 #{}", self.version));
            });
            if let Some(notice) = &self.comparison.notice {
                ui.colored_label(theme::WARNING(), notice);
            }
            if self.play_confirmation.is_some() {
                ui.label("等待确认共同稿件范围；未应用输入完整保留");
            }
            let compared = self.comparison.result.take();
            if let Some(compared) = &compared {
                ui.label(theme::muted(format!(
                    "以下 A/B 结果均验证于已应用稿 #{}",
                    compared.scope.version
                )));
                if !compared.selections_match(self.comparison.a, self.comparison.b) {
                    ui.colored_label(
                        theme::WARNING(),
                        "所选路径已变化；以下仍是上次 A/B 结果，请显式重新比较",
                    );
                }
                ui.columns(2, |columns| {
                    for (index, ui) in columns.iter_mut().enumerate() {
                        route_header(
                            ui,
                            if index == 0 { "A" } else { "B" },
                            compared.name(index == 1),
                            Some(compared.side(index == 1)),
                            false,
                        );
                    }
                });
                if !source_current {
                    ui.colored_label(
                        theme::WARNING(),
                        "结果已过期，仍属于旧稿；请重新比较。旧来源不可跳转",
                    );
                }
                if source_current {
                    if let Some(reason) = blocked.as_deref() {
                        ui.colored_label(theme::WARNING(), reason);
                    }
                }
                if !compared.scope.excluded_inputs.is_empty() {
                    ui.colored_label(
                        theme::WARNING(),
                        format!(
                            "本次未纳入 {} 项作者草稿；草稿仍保留",
                            compared.scope.excluded_inputs.len()
                        ),
                    );
                }
                if compared.result.omitted || !compared.result.differences_complete {
                    ui.colored_label(
                        theme::WARNING(),
                        "结果或动作证据不完整；遗漏不能解释为没有差异或没有发生",
                    );
                }
                ui.add_space(theme::SPACE_SM);
                first_difference(ui, compared, &access, &mut request);
                ui.separator();
                super::focus::restore(ctx, &mut self.comparison.restore_focus);
                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt("route-comparison-body")
                    .auto_shrink([false, false])
                    // 快速 Tab 时上一控件的滚动动画不能继续把新焦点带出窄视口。
                    // 仅关闭程序化目标动画；手动滚动仍由原生 ScrollArea 处理。
                    .animated(false);
                if self.comparison.restore_scroll {
                    // 与作者书稿返回相同：旧焦点目标和惯性不能覆盖明确保存的位置。
                    let mut state = egui::scroll_area::State::default();
                    state.offset.y = self.comparison.scroll;
                    state.store(
                        ctx,
                        ui.make_persistent_id(egui::Id::new("route-comparison-body")),
                    );
                    scroll = scroll
                        .vertical_scroll_offset(self.comparison.scroll)
                        .animated(false);
                    self.comparison.restore_scroll = false;
                }
                let output = scroll.show(ui, |ui| {
                    super::details::render(
                        ui,
                        compared,
                        &mut self.comparison,
                        &access,
                        &mut request,
                        &mut target,
                        self.snapshot
                            .as_ref()
                            .filter(|_| source_current)
                            .map(|snapshot| &snapshot.result.analysis.catalog),
                    );
                    self.comparison_technical(ui, Some(compared));
                    if restoring {
                        ui.scroll_to_rect(ui.clip_rect(), None);
                    }
                });
                self.comparison.scroll = output.state.offset.y;
            } else {
                ui.columns(2, |columns| {
                    for (index, ui) in columns.iter_mut().enumerate() {
                        let name = self
                            .comparison
                            .job
                            .as_ref()
                            .map(|job| job.names[index].as_str())
                            .or_else(|| {
                                [self.comparison.a, self.comparison.b][index]
                                    .and_then(|i| self.replay_debugger.saved_paths.get(i))
                                    .map(|p| p.name.as_str())
                            })
                            .unwrap_or("尚未选择路径");
                        route_header(ui, if index == 0 { "A" } else { "B" }, name, None, running);
                    }
                });
                ui.label(if running {
                    "比较正在独立临时运行中；普通试玩及作者草稿保持原状"
                } else {
                    "先录制两条路径，或展开下面的路径 JSON 导入；比较不会推进普通试玩"
                });
                egui::ScrollArea::vertical()
                    .id_salt("comparison-empty")
                    .show(ui, |ui| {
                        self.comparison_technical(ui, None);
                    });
            }
            self.comparison.result = compared;
        });
        if run {
            self.request_comparison(ctx);
        }
        if let Some(request) = request {
            self.jump_to_comparison_source(ctx, &request);
        }
        if let Some(target) = target {
            self.open_reading(target);
        }
    }

    fn comparison_technical(&mut self, ui: &mut egui::Ui, compared: Option<&ComparedRoutes>) {
        egui::CollapsingHeader::new("运行详情与预算")
            .id_salt("comparison-technical")
            .show(ui, |ui| {
                ui.label("比较上限：每条 4 MiB / 4,096 选择；两侧共用步数与时间预算");
                crate::theme::add_enabled_ui(ui, self.comparison.job.is_none(), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("合计步数");
                        ui.add(
                            egui::DragValue::new(&mut self.comparison.max_steps).range(0..=100_000),
                        );
                        ui.label("合计毫秒");
                        ui.add(
                            egui::DragValue::new(&mut self.comparison.time_budget_ms)
                                .range(0..=30_000),
                        );
                    });
                });
                #[cfg(target_arch = "wasm32")]
                ui.label(
                    "浏览器单帧至多 512 步 / 4 ms，总时限至多 2,000 ms；单语句和起点恢复不可抢占",
                );
                if let Some(compared) = compared {
                    ui.label(format!(
                        "runtime {} · schema {} · fingerprint {}",
                        compared.result.runtime_version,
                        compared.result.schema_version,
                        compared.result.source_fingerprint
                    ));
                    ui.label(format!("源快照 {}", compared.result.source_snapshot));
                    super::super::scope::render_scope(ui, &compared.scope);
                    for side in [&compared.result.left, &compared.result.right] {
                        ui.label(format!(
                            "原 fingerprint {} · 执行 {} 步 · 验证 {} 次选择",
                            side.original_fingerprint, side.executed_steps, side.completed_choices
                        ));
                    }
                }
            });
        super::super::debugger::render_trace_import(ui, &mut self.replay_debugger);
    }
}
