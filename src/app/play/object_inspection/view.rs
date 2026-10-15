use super::*;
use crate::{app::writing_workspace::preview_navigation as shared, theme};
use worldline_runtime::{InspectionBaseline, StateInspectionItem};

impl WorldeditApp {
    pub(in crate::app) fn object_inspection_window(&mut self, ctx: &egui::Context) {
        let composing = self.object_inspection_composing(ctx);
        if !self.replay_debugger.object_inspection.open {
            keyboard::finish_closed(self, ctx);
            return;
        }
        self.refresh_object_inspection(ctx);
        if !self.replay_debugger.object_inspection.open {
            keyboard::finish_closed(self, ctx);
            return;
        }
        self.sync_edit_layers(ctx);
        let top = self.edit_layer_is_top("object-inspection")
            && ctx.memory(|memory| memory.top_modal_layer().is_none());
        // 全局快捷键仍保守拦截 IME 生命周期；此处只接管本窗无真实组合时的 Escape。
        if top
            && ctx.input(|input| input.focused)
            && !composing
            && !egui::Popup::is_any_open(ctx)
            && !self.source_mention_owns_escape(ctx)
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.replay_debugger.object_inspection.open = false;
            self.replay_debugger.object_inspection.return_focus_pending = true;
            self.sync_edit_layers(ctx);
            keyboard::finish_closed(self, ctx);
            return;
        }
        let active =
            top && ctx.input(|input| input.focused) && !egui::Popup::is_any_open(ctx) && !composing;
        shared::transient_host(ctx, Some((keyboard::layer(), active)));
        let ordinary = self
            .play
            .as_ref()
            .and_then(|play| play.story.as_ref())
            .is_some();
        let draft = self
            .object_rehearsal_snapshot()
            .is_some_and(|snapshot| snapshot.stamp.is_some());
        let mut state = std::mem::take(&mut self.replay_debugger.object_inspection);
        let mut open = true;
        let mut return_origin = false;
        let mut choose_run = None;
        let mut source = None;
        let mut evidence_jump = None;
        let screen = ctx.screen_rect();
        let menu_active = top && ctx.input(|input| input.focused) && !composing;
        // 在本窗 FocusCycle 消费 Tab 前记录这次显露许可。
        let keyboard_reveal = crate::chrome::action_layout::keyboard_navigation_active(ctx);
        state.run_menu.advance(ctx, menu_active);
        state.keyboard.advance(ctx, active);
        let mut reader = None;
        let mut content_ids = Vec::new();
        let window = egui::Window::new("对象 · 真实运行状态旁查")
            .id(egui::Id::new("object-state-inspection"))
            .order(if top {
                egui::Order::Foreground
            } else {
                egui::Order::Background
            })
            .open(&mut open)
            .resizable(true)
            .default_width((screen.width() - 40.0).clamp(240.0, 820.0))
            .default_height((screen.height() - 80.0).clamp(200.0, 640.0))
            .max_size((screen.size() - egui::vec2(20.0, 30.0)).max(egui::vec2(180.0, 120.0)))
            .constrain_to(screen.shrink(8.0))
            .show(ctx, |ui| {
                if !top {
                    ui.disable();
                }
                let target = state.target.as_ref().unwrap();
                let display = state
                    .page
                    .as_ref()
                    .and_then(|page| page.owner.as_ref())
                    .map(|owner| owner.display.as_str())
                    .unwrap_or(&target.id);
                ui.add(
                    egui::Label::new(format!("{display} · {}:{}", target.kind, target.id))
                        .truncate(),
                )
                .on_hover_text(format!("{}:{}", target.kind, target.id));
                ui.horizontal_wrapped(|ui| {
                    let back = ui
                        .push_id("object-inspection-return", |ui| {
                            theme::add_enabled(ui, !composing, egui::Button::new("返回对象入口"))
                        })
                        .inner;
                    state.keyboard.initial(&back, active);
                    shared::reveal(ui, &back, true, true);
                    crate::chrome::action_layout::record(ui, "object/return", &back);
                    return_origin |= back.clicked();
                    let picker = ui.menu_button("选择已有运行", |ui| {
                        let mut items = Vec::new();
                        for (kind, available) in
                            [(RunKind::Ordinary, ordinary), (RunKind::Draft, draft)]
                        {
                            let run = theme::add_enabled(
                                ui,
                                available && !composing,
                                egui::Button::new(kind.label()),
                            );
                            crate::chrome::action_layout::record(
                                ui,
                                match kind {
                                    RunKind::Ordinary => "object/run/ordinary",
                                    RunKind::Draft => "object/run/draft",
                                },
                                &run,
                            );
                            if run.clicked() {
                                choose_run = Some(kind);
                                ui.close();
                            }
                            items.push(run);
                        }
                        ui.label("只查询现存运行；不会开始或重开故事");
                        items
                    });
                    crate::chrome::action_layout::record(ui, "object/run-picker", &picker.response);
                    state
                        .run_menu
                        .finish(&picker.response, picker.inner.as_deref(), menu_active);
                    if let Some(kind) = state.run {
                        ui.label(format!("{} · 稿 #{}", kind.label(), state.version));
                    }
                });
                if let Some(stamp) = state.stamp {
                    let status = state
                        .page
                        .as_ref()
                        .map(|page| super::super::inspection::status(page.status))
                        .unwrap_or(if state.error.is_some() {
                            "查询不可用"
                        } else {
                            "正在查询"
                        });
                    ui.add(
                        egui::Label::new(format!(
                            "运行 {} · 观测代次 {} · {status}",
                            stamp.run_id, stamp.revision
                        ))
                        .wrap(),
                    );
                }
                if state.run_changed {
                    ui.colored_label(
                        theme::WARNING(),
                        "运行已推进、重开或关闭；此页只读保留。请明确选择已有运行重新查询",
                    );
                }
                if let Some(reason) = &state.source_reason {
                    ui.colored_label(theme::WARNING(), reason);
                }
                if state.page.as_ref().is_some_and(|page| page.history_omitted) {
                    ui.colored_label(theme::WARNING(), "部分历史已省略，不能补零或推断完整路径");
                }
                if let Some(error) = &state.error {
                    ui.colored_label(theme::ERROR(), error);
                }
                if state.awaiting {
                    ui.label("正在读取同一草稿运行；旧页保留");
                }
                ui.separator();
                let body_height = (ui.available_height() - 8.0).max(36.0);
                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt((
                        "object-inspection-content",
                        state.run,
                        state.stamp.map(|stamp| stamp.run_id),
                    ))
                    .max_height(body_height)
                    .auto_shrink([false, false]);
                if state.restore_scroll {
                    scroll = scroll.vertical_scroll_offset(state.scroll);
                    state.restore_scroll = false;
                }
                let output = scroll.show_viewport(ui, |ui, relative| {
                    let outside: Vec<_> = ctx.viewport(|v| {
                        v.this_pass
                            .widgets
                            .get_layer(ui.layer_id())
                            .map(|widget| widget.id)
                            .collect()
                    });
                    let viewport = shared::viewport(ui, relative).intersect(ui.clip_rect());
                    let mut reader_delta = egui::Vec2::ZERO;
                    if state.run.is_some() {
                        let (response, delta) = shared::reading_deferred(
                            ui,
                            shared::Reader {
                                identity: egui::Id::new("object-state-reader"),
                                label: "运行状态阅读区 · ↑↓滚动",
                                viewport,
                                enabled: active,
                                horizontal: false,
                                vertical: true,
                                step: egui::vec2(
                                    0.0,
                                    ui.text_style_height(&egui::TextStyle::Body) * 3.0,
                                ),
                                escape: true,
                            },
                        );
                        reader = Some(response.id);
                        reader_delta = delta;
                    }
                    if state.run.is_none() {
                        ui.label(if ordinary || draft {
                            "请选择一份已有运行，查看这个完整对象身份所拥有的状态"
                        } else {
                            "当前没有普通试玩或草稿试演；旁查不会自动启动故事"
                        });
                        ui.label("静态资料仍在原入口；运行状态与初始声明分开呈现");
                        return;
                    }
                    ui.collapsing("完整对象与来源身份", |ui| {
                        ui.label(format!("对象：{}:{}", target.kind, target.id));
                        ui.label(format!("实际来源：{}", state.baseline));
                        if let Some(stamp) = state.stamp {
                            ui.label(format!(
                                "compiled snapshot {} · fingerprint {} · trace {} · revision {}",
                                stamp.compiled_snapshot,
                                stamp.fingerprint,
                                stamp.trace_generation,
                                stamp.revision
                            ));
                        }
                        ui.label(
                            "对象名称、所属状态和声明初值来自这次运行冻结的目录，不与新稿拼接",
                        );
                    });
                    let before = state.query.clone();
                    theme::add_enabled_ui(ui, !state.run_changed, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut state.query.text)
                                .id(egui::Id::new("object-inspection-search"))
                                .hint_text("筛选此对象的状态或当前值")
                                .char_limit(256)
                                .desired_width(f32::INFINITY),
                        );
                        theme::add_enabled_ui(ui, !composing, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.checkbox(&mut state.query.changed_only, "只看变化");
                                ui.selectable_value(
                                    &mut state.query.compare_to,
                                    InspectionBaseline::First,
                                    "比较首次记录",
                                );
                                ui.selectable_value(
                                    &mut state.query.compare_to,
                                    InspectionBaseline::Previous,
                                    "比较上次记录",
                                );
                            });
                        });
                    });
                    if state.query != before {
                        state.query.offset = 0;
                    }
                    let Some(page) = &state.page else {
                        return;
                    };
                    ui.label(super::super::inspection::observation_note(page));
                    ui.label("首次是实际观察记录，可能已发生动作；不等于静态初始声明");
                    ui.label(format!(
                        "所属状态 {} 项 · 匹配 {} 项 · 不可比较 {} 项",
                        page.total_items, page.total_matches, page.incomparable_items
                    ));
                    if page.total_items == 0 {
                        ui.label("这个对象在所选运行中没有所属状态；不会猜测普通变量或对象属性");
                    } else if page.items.is_empty() {
                        ui.label("当前筛选没有匹配的所属状态");
                    }
                    ui.horizontal_wrapped(|ui| {
                        if theme::add_enabled(
                            ui,
                            !composing && !state.run_changed && !state.awaiting && page.offset > 0,
                            egui::Button::new("上一页"),
                        )
                        .clicked()
                        {
                            state.query.offset = page.offset.saturating_sub(page.limit);
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
                            !composing
                                && !state.run_changed
                                && !state.awaiting
                                && page.next_offset.is_some(),
                            egui::Button::new("下一页"),
                        )
                        .clicked()
                        {
                            state.query.offset = page.next_offset.unwrap_or(0);
                        }
                    });
                    for item in &page.items {
                        ui.push_id(&item.key, |ui| {
                            ui.group(|ui| {
                                let title = item
                                    .state_definition
                                    .as_ref()
                                    .map(|definition| {
                                        format!("{} · {}", definition.display, item.key.name)
                                    })
                                    .unwrap_or_else(|| item.key.name.clone());
                                let select = ui.selectable_label(
                                    state.selected.as_ref() == Some(&item.key),
                                    title,
                                );
                                crate::chrome::action_layout::record(
                                    ui,
                                    &format!("object/state/{}", item.key.name),
                                    &select,
                                );
                                if select.clicked() {
                                    state.selected = Some(item.key.clone());
                                }
                                ui.label(super::super::inspection::change_label(
                                    if state.query.compare_to == InspectionBaseline::First {
                                        item.first_change
                                    } else {
                                        item.previous_change
                                    },
                                ));
                                values(ui, item);
                                if let Some(definition) = &item.state_definition {
                                    ui.collapsing("这次运行的静态初始声明", |ui| {
                                        super::super::inspection::cell(
                                            ui,
                                            "声明初值（未执行）",
                                            &definition.initial,
                                        );
                                        ui.label(format!(
                                            "所属 {}:{}",
                                            definition.owner.kind, definition.owner.id
                                        ));
                                    });
                                }
                                let can_source = !state.run_changed
                                    && state.source_reason.is_none()
                                    && !state.awaiting
                                    && !composing;
                                let declaration = theme::add_enabled(
                                    ui,
                                    can_source && item.source.is_some(),
                                    egui::Button::new("定位真实状态声明"),
                                );
                                crate::chrome::action_layout::record(
                                    ui,
                                    &format!("object/source/{}", item.key.name),
                                    &declaration,
                                );
                                if declaration.clicked() {
                                    source = item.source.clone().map(|source| {
                                        super::super::inspection::SourceRequest {
                                            stamp: page.stamp,
                                            key: item.key.clone(),
                                            source,
                                        }
                                    });
                                }
                                ui.label(theme::muted("声明头不是最后写入；值差异不推断因果"));
                            });
                        });
                    }
                    ui.collapsing("同次运行已记录的条件证据", |ui| {
                        ui.label("以下是这次运行的当前选择证据；不据状态终值猜测对象因果");
                        let reason = state.source_reason.clone().or_else(|| {
                            state
                                .run_changed
                                .then(|| "运行身份已变化；旧证据只读保留".into())
                        });
                        let access =
                            super::super::evidence_navigation::EvidenceNavigationAccess::rehearsal(
                                &state.evidence,
                                reason,
                            );
                        match state.run {
                            Some(RunKind::Ordinary) => super::super::evidence::render(
                                ui,
                                &state.evidence,
                                state.version,
                                self.version,
                                &access,
                                &mut evidence_jump,
                            ),
                            _ => super::super::evidence::render_rehearsal(
                                ui,
                                &state.evidence,
                                &access,
                                &mut evidence_jump,
                            ),
                        }
                    });
                    if top {
                        keyboard::reveal(ui, reader, &outside, active, keyboard_reveal);
                    }
                    content_ids = ctx.viewport(|v| {
                        v.this_pass
                            .widgets
                            .get_layer(ui.layer_id())
                            .map(|widget| widget.id)
                            .filter(|id| !outside.contains(id))
                            .collect()
                    });
                    if reader_delta != egui::Vec2::ZERO {
                        ui.scroll_with_delta(reader_delta);
                    }
                });
                state.scroll = output.state.offset.y;
                state.focus = ctx.memory(|memory| memory.focused());
            });
        if top {
            if let Some(window) = window.as_ref() {
                shared::outline_shell(ctx, keyboard::layer(), window.response.rect, &content_ids);
            }
        }
        state.keyboard.finish(
            ctx,
            keyboard::layer(),
            active && !egui::Popup::is_any_open(ctx),
            reader,
        );
        if composing && !open {
            if let Some(focus) = state.focus {
                ctx.memory_mut(|memory| memory.request_focus(focus));
            }
        }
        state.open = (open || composing) && !return_origin;
        state.return_focus_pending = !state.open;
        self.replay_debugger.object_inspection = state;
        if let Some(kind) = choose_run {
            self.select_object_run(kind);
        }
        if let Some(request) = source {
            self.jump_to_object_inspection_source(ctx, &request);
        }
        if let Some(request) = evidence_jump {
            self.jump_to_object_evidence(ctx, &request);
        }
        if return_origin {
            if let Some(origin) = self.replay_debugger.object_inspection.origin.take() {
                self.restore_author_location(*origin, ctx);
            }
        }
        if !self.replay_debugger.object_inspection.open {
            keyboard::finish_closed(self, ctx);
        }
    }
}

fn values(ui: &mut egui::Ui, item: &StateInspectionItem) {
    let captions = ["首次记录", "上次记录", "当前真实值"];
    let font = egui::TextStyle::Body.resolve(ui.style());
    let width = captions
        .iter()
        .map(|caption| {
            ui.painter()
                .layout_no_wrap((*caption).into(), font.clone(), ui.visuals().text_color())
                .size()
                .x
        })
        .fold(170.0_f32, f32::max)
        + 24.0;
    if ui.available_width() >= width * 3.0 + ui.spacing().item_spacing.x * 2.0 {
        ui.columns(3, |columns| {
            for (column, (label, value)) in columns.iter_mut().zip(captions.into_iter().zip([
                &item.first,
                &item.previous,
                &item.current,
            ])) {
                super::super::inspection::cell(column, label, value);
            }
        });
    } else {
        for (label, value) in [
            ("当前真实值", &item.current),
            ("上次记录", &item.previous),
            ("首次记录", &item.first),
        ] {
            super::super::inspection::cell(ui, label, value);
        }
    }
}
