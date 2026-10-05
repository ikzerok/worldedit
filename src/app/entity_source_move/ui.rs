//! 名称、稳定身份与完整路径在可滚动正文；确认/取消固定在窗口底部。
use super::*;
use crate::theme;
impl WorldeditApp {
    pub(in crate::app) fn entity_source_move_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.entity_source_move_form.take() else {
            return;
        };
        let mut open = true;
        let mut cancel = false;
        let mut preview = false;
        let mut apply = false;
        let targets = self.entity_move_targets();
        let ime_busy = self.ime_composing
            || self.command_palette.ime
            || self.command_palette.ime_frame
            || ctx.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Ime(_)))
            });
        let mut focus_target = form.focus_on_open;
        let list_focused = ctx
            .memory(|memory| memory.focused())
            .is_some_and(|id| form.target_focus_ids.contains(&id));
        if list_focused && self.edit_layer_is_top("entity-source-move") && !ime_busy {
            let down = ctx
                .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
            let up =
                ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
            if (up || down) && !targets.is_empty() {
                let current = targets
                    .iter()
                    .position(|path| *path == form.destination)
                    .unwrap_or(0);
                let next = if down {
                    (current + 1) % targets.len()
                } else {
                    (current + targets.len() - 1) % targets.len()
                };
                form.select(targets[next].clone());
                focus_target = true;
            }
            preview =
                ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Tab)) {
                if let Some(id) = form.preview_focus {
                    ctx.memory_mut(|memory| memory.request_focus(id));
                }
            } else if ctx
                .input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab))
            {
                if let Some(id) = form.cancel_focus {
                    ctx.memory_mut(|memory| memory.request_focus(id));
                }
            }
        }
        form.target_focus_ids.clear();
        let current = form.current(self);
        // IME 只抑制动作；临时禁用获焦控件会让 egui 丢失焦点。
        let can_apply = current
            && form
                .plan
                .as_ref()
                .is_some_and(|plan| !plan.changes.is_empty())
            && (ime_busy || self.entity_source_move_blocker().is_none());
        let viewport = ctx.available_rect().shrink(8.0);
        let style = ctx.style();
        let frame = egui::Frame::window(&style);
        let title_height = ctx
            .fonts(|fonts| fonts.row_height(&style.text_styles[&egui::TextStyle::Heading]))
            .max(style.spacing.interact_size.y);
        let chrome = frame.total_margin().sum().y
            + title_height
            + frame.inner_margin.sum().y
            + frame.stroke.width;
        egui::Window::new("移动实体资料声明")
            .id(egui::Id::new("entity-source-move"))
            .open(&mut open).collapsible(false).resizable(true)
            .default_width(680.0).default_height(620.0)
            .max_width((viewport.width() - frame.total_margin().sum().x).max(240.0))
            .max_height((viewport.height() - chrome).max(180.0))
            .constrain_to(viewport).vscroll(false).show(ctx, |ui| {
                egui::TopBottomPanel::bottom("entity-source-move-actions")
                    .frame(egui::Frame::NONE).resizable(false).show_inside(ui, |ui| {
                        ui.separator();
                        // 状态只有一行；详细原因留在独立滚动区，不能挤走动作。
                        let status = if form.error.is_some() { "未应用 · 原选择与作品保留" }
                            else if form.plan.is_some() && !current { "预览过期 · 请重新预览" }
                            else if form.plan.as_ref().is_some_and(|p| p.changes.is_empty()) { "同一来源 · 无变化，不新增撤销" }
                            else { "只读预览 · 应用后仍需保存全部" };
                        ui.label(status);
                        ui.horizontal_wrapped(|ui| {
                            let response = ui.add_enabled(form.root == self.project.root, egui::Button::new("预览实体移源"));
                            form.preview_focus = Some(response.id);
                            preview |= response.clicked() && !ime_busy;
                            let response = ui.add_enabled(can_apply, theme::primary("应用实体移源"));
                            form.apply_focus = Some(response.id);
                            apply = response.clicked() && !ime_busy;
                            let response = ui.button("取消移源");
                            form.cancel_focus = Some(response.id);
                            cancel = response.clicked() && !ime_busy;
                        });
                    });
                egui::ScrollArea::vertical().id_salt("entity-source-move-content")
                    .auto_shrink([false, false]).show(ui, |ui| {
                        ui.heading(&form.display);
                        wrapped(ui, &format!("稳定 ID：entity:{}", form.id));
                        wrapped(ui, &format!("当前来源：{}", theme::relative_source(&form.root, &form.source)));
                        wrapped(ui, "只移动这个 entity 声明及 core 确认属于它的说明、属性与块内注释。人物、事件、片段及其他声明保持原位。不会自动创建文件或启用能力。");
                        ui.separator();
                        ui.label("选择已有活动 .wl 源码");
                        wrapped(ui, &format!("目标完整路径：{}", theme::relative_source(&form.root, &form.destination)));
                        wrapped(ui, "↑↓选择目标，Enter预览；Tab离开清单，Esc取消");
                        egui::CollapsingHeader::new("展开目标源码清单")
                            .default_open(true).show(ui, |ui| {
                                egui::ScrollArea::vertical().id_salt("entity-source-target-list").max_height(180.0).show(ui, |ui| {
                                for path in &targets {
                                    let label = format!("{}{}", theme::relative_source(&form.root, path),
                                        if *path == form.source { "（当前位置）" } else { "" });
                                    let response = ui.add(egui::Button::selectable(*path == form.destination, label).wrap());
                                    form.target_focus_ids.push(response.id);
                                    if response.has_focus() {
                                        ui.memory_mut(|memory| memory.set_focus_lock_filter(response.id,
                                            egui::EventFilter { tab: true, vertical_arrows: true, ..Default::default() }));
                                    }
                                    if focus_target && *path == form.destination {
                                        response.request_focus();
                                        response.scroll_to_me(None);
                                        form.focus_on_open = false;
                                    }
                                    if response.clicked() && !ime_busy { form.select(path.clone()); }
                                }
                                });
                            });
                        wrapped(ui, "需要新文件？取消后使用工程文件树“工程文件 ＋”，再回到本实体移源。");
                        if let Some(error) = &form.error {
                            ui.separator();
                            let advice = match form.failure_kind {
                                Some(SourceLifecycleFailureKind::IllegalPath) => "请重新选择已有活动源码；归档、删除文件和工作区外路径不可用。",
                                Some(SourceLifecycleFailureKind::SemanticChange) => "移动会改变语义；请核对来源与目标，处理原因后重新预览。",
                                Some(SourceLifecycleFailureKind::UnableToProve) => "无法证明安全；请先处理诊断、能力或工作区保护问题。",
                                _ => "选择和其他草稿均保留；处理未应用输入或工程变化后重新预览。",
                            };
                            ui.colored_label(theme::ERROR(), "未应用");
                            wrapped(ui, advice);
                            wrapped(ui, error);
                        }
                        if let Some(plan) = &form.plan {
                            let heading = ui.heading("实体移源 · 精确原文预览");
                            wrapped(ui, &format!("对象：{} · entity:{}", form.display, form.id));
                            if let (Some(source), Some(destination)) = (&plan.source_path, &plan.destination_path) {
                                wrapped(ui, &format!("{} → {}", theme::relative_source(&form.root, source), theme::relative_source(&form.root, destination)));
                            }
                            draw_entity_plan(ui, plan, &form.root);
                            if form.reveal_preview {
                                ui.scroll_to_rect(heading.rect, Some(egui::Align::TOP));
                                form.reveal_preview = false;
                            }
                        }
                        else { wrapped(ui, "预览由 core 给出源删除、目标插入的精确原文范围；预览和取消不改变作品。"); }
                    });
            });
        if preview {
            self.preview_entity_source_move(&mut form);
            if let Some(id) = if form
                .plan
                .as_ref()
                .is_some_and(|plan| !plan.changes.is_empty())
            {
                form.apply_focus
            } else {
                form.preview_focus
            } {
                ctx.memory_mut(|memory| memory.request_focus(id));
            }
        }
        let applied = apply && self.apply_entity_source_move(&mut form);
        if (open || ime_busy) && !cancel && !applied {
            self.entity_source_move_form = Some(form);
        }
    }
}
fn wrapped(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(text).wrap().selectable(true));
}
fn raw_text(ui: &mut egui::Ui, text: &str) {
    if text.is_empty() {
        wrapped(ui, "（空；不含原文字节）");
    } else {
        ui.add(
            egui::Label::new(egui::RichText::new(text).monospace())
                .wrap()
                .selectable(true),
        );
    }
}
fn draw_entity_plan(ui: &mut egui::Ui, plan: &SourceLifecyclePlan, root: &std::path::Path) {
    ui.separator();
    if plan.changes.is_empty() {
        wrapped(ui, "core 确认是同一来源：没有需要应用的修改。");
        return;
    }
    wrapped(
        ui,
        &format!("{} 份源码变更 · 只读精确原文预览", plan.changes.len()),
    );
    let mut changes: Vec<_> = plan.changes.iter().collect();
    changes.sort_by_key(|change| Some(&change.path) != plan.source_path.as_ref());
    for change in changes {
        let removing = Some(&change.path) == plan.source_path.as_ref();
        ui.heading(if removing {
            "源文件：删除"
        } else {
            "目标文件：插入"
        });
        wrapped(ui, &theme::relative_source(root, &change.path));
        for occurrence in &change.occurrences {
            wrapped(
                ui,
                &format!(
                    "core 第 {} 行 · UTF-8 字节 [{}..{}) → [{}..{})",
                    occurrence.line,
                    occurrence.before_range.start,
                    occurrence.before_range.end,
                    occurrence.after_range.start,
                    occurrence.after_range.end
                ),
            );
            ui.label("修改前原文");
            raw_text(ui, &occurrence.before_token);
            ui.label("修改后原文");
            raw_text(ui, &occurrence.after_token);
            egui::CollapsingHeader::new("核对前后上下文")
                .id_salt((&change.path, occurrence.before_range.start))
                .show(ui, |ui| {
                    ui.label("修改前上下文");
                    raw_text(ui, &occurrence.before_context);
                    ui.label("修改后上下文");
                    raw_text(ui, &occurrence.after_context);
                });
        }
    }
    egui::CollapsingHeader::new("core 等价证明").show(ui, |ui| {
        wrapped(ui, &format!("活动身份：{}", plan.membership));
        wrapped(
            ui,
            &format!(
                "运行指纹：{:016x} → {:016x}",
                plan.runtime_fingerprint_before, plan.runtime_fingerprint_after
            ),
        );
        wrapped(
            ui,
            &format!("默认入口：{} → {}", plan.entry_before, plan.entry_after),
        );
        for (before, after) in plan.load_order_before.iter().zip(&plan.load_order_after) {
            wrapped(ui, &format!("加载顺序：{before} → {after}"));
        }
        for resource in &plan.resources {
            wrapped(
                ui,
                &format!(
                    "资源 {}：{} → {} · {}",
                    resource.field,
                    resource.before_path,
                    resource.after_path,
                    resource.content_digest
                ),
            );
        }
    });
}
