use super::*;

pub(super) fn draw(
    ui: &mut egui::Ui,
    project: &Project,
    buffer: &mut WritingBuffer,
    target: &TargetRef,
    view: &mut ViewState,
    typography: Typography,
    action: &mut Action,
) {
    if view.mode == Mode::Source {
        if !typography.compact {
            ui.label(theme::muted(
                "完整源码；与写作和结构视图共用一个文件草稿。无效输入不会丢失。",
            ));
        }
        let key = prose::RetainedKey::new(buffer.path(), target, 0);
        let generation = buffer.generation();
        let mut source = view.prose_text(buffer.path(), target, 0, buffer.source());
        let id = egui::Id::new(("writing-source", buffer.path(), &target.kind, &target.id));
        if !view.pending_prose(&key) {
            view.restore_editor(ui, id, buffer, 0, &source);
        }
        let context_selection = super::context_selection::SelectionPress::capture(
            ui, id, view, buffer, target, 0, &source,
        );
        let mut output = egui::TextEdit::multiline(&mut source)
            .id(id)
            .code_editor()
            .font(theme::source_font(typography.source_size))
            .desired_width(f32::INFINITY)
            .desired_rows(18)
            .show(ui);
        if let Some(selection) = context_selection {
            selection.restore(ui, &mut output, buffer);
        }
        action.error = super::composition_text::finish(
            view,
            key,
            &source,
            generation,
            output.response.changed(),
            || {
                buffer.replace_source(source.clone());
                Ok(())
            },
        );
        view.record_cursor(ui, &output, buffer, target, 0, &source);
        link_context_menu(&output.response, view, action);
        view.pending_cursor = None;
        retained_notice(ui, view);
        return;
    }
    let projection = match view.projection_cache.get(project, buffer, target) {
        Ok(projection) => projection,
        Err(error) => {
            view.pending_cursor = None;
            ui.colored_label(theme::ERROR(), &error);
            super::composition_text::rescue(ui, buffer, target, view, typography, &error, action);
            if ui.button("在源码视图继续编辑").clicked() {
                view.restore_mode(Mode::Source);
            }
            retained_notice(ui, view);
            return;
        }
    };
    if view.pending_outside_projection(buffer, target, &projection) {
        let error = "正文范围在组合期间移动；当前提交会保留供核对，不覆盖新稿";
        ui.colored_label(theme::ERROR(), error);
        super::composition_text::rescue(ui, buffer, target, view, typography, error, action);
        retained_notice(ui, view);
        return;
    }
    if view.mode == Mode::Structure {
        if !typography.compact {
            ui.label(theme::muted(
                "仅当前目标声明体；保留缩进、注释与所有复杂控制语句。",
            ));
        }
        let offset = projection.range.start;
        let key = prose::RetainedKey::new(buffer.path(), target, offset);
        let mut source = view.prose_text(buffer.path(), target, offset, &projection.source);
        let id = egui::Id::new(("writing-structure", buffer.path(), &target.kind, &target.id));
        if !view.pending_prose(&key) {
            view.restore_editor(ui, id, buffer, offset, &source);
        }
        let context_selection = super::context_selection::SelectionPress::capture(
            ui, id, view, buffer, target, offset, &source,
        );
        let mut output = egui::TextEdit::multiline(&mut source)
            .id(id)
            .code_editor()
            .font(theme::source_font(typography.source_size))
            .desired_width(f32::INFINITY)
            .desired_rows(18)
            .show(ui);
        if let Some(selection) = context_selection {
            selection.restore(ui, &mut output, buffer);
        }
        action.error = super::composition_text::finish(
            view,
            key,
            &source,
            projection.generation,
            output.response.changed(),
            || {
                buffer.replace_range(
                    projection.generation,
                    projection.range.clone(),
                    &projection.source,
                    &source,
                )
            },
        );
        view.record_cursor(ui, &output, buffer, target, offset, &source);
        link_context_menu(&output.response, view, action);
        view.pending_cursor = None;
        retained_notice(ui, view);
        return;
    }
    if !typography.compact {
        ui.label(theme::muted(
            "写下正文；链接、内插与行标记保留为源文。复杂控制语句可在结构或源码中编辑。",
        ));
    }
    if let Some(slot) = &projection.empty_prose_slot {
        let key = prose::RetainedKey::new(buffer.path(), target, slot.offset());
        let mut text = view.prose_text(buffer.path(), target, slot.offset(), slot.text());
        let output = prose::edit(
            ui,
            buffer,
            target,
            view,
            typography,
            slot.offset(),
            &mut text,
            true,
        );
        action.error = super::composition_text::finish(
            view,
            key,
            &text,
            projection.generation,
            output.response.changed(),
            || project.insert_writing_prose(buffer, slot, &text),
        );
        view.record_cursor(ui, &output, buffer, target, slot.offset(), &text);
        // 首次插入后不在同帧再绘制普通块，否则同一 TextEdit ID 会重复。
        if output.response.changed() {
            view.pending_cursor = None;
            retained_notice(ui, view);
            return;
        }
    } else if projection
        .blocks
        .iter()
        .all(|block| block.kind != worldline_core::manuscript::WritingBlockKind::Prose)
    {
        ui.label("此来源不能安全推断空正文位置，请选择结构或源码继续写作。");
        if crate::theme::add_enabled(ui, !view.ime_active, egui::Button::new("打开结构编辑"))
            .clicked()
        {
            view.restore_mode(Mode::Structure);
        }
    }
    for block in &projection.blocks {
        match block.kind {
            worldline_core::manuscript::WritingBlockKind::Prose => {
                let key = prose::RetainedKey::new(buffer.path(), target, block.range.start);
                let mut text =
                    view.prose_text(buffer.path(), target, block.range.start, &block.text);
                let output = prose::edit(
                    ui,
                    buffer,
                    target,
                    view,
                    typography,
                    block.range.start,
                    &mut text,
                    false,
                );
                action.error = super::composition_text::finish(
                    view,
                    key,
                    &text,
                    projection.generation,
                    output.response.changed(),
                    || buffer.replace_prose(projection.generation, block, &text),
                );
                view.record_cursor(ui, &output, buffer, target, block.range.start, &text);
                link_context_menu(&output.response, view, action);
                if output.response.changed() || buffer.generation() != projection.generation {
                    break;
                }
            }
            worldline_core::manuscript::WritingBlockKind::Structure if !block.text.is_empty() => {
                egui::CollapsingHeader::new(theme::muted(&block.label))
                    .id_salt(("writing-structure-detail", buffer.path(), block.range.start))
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&block.text)
                                    .font(theme::source_font(typography.source_size)),
                            )
                            .wrap(),
                        );
                    });
            }
            _ => {
                ui.add_space(theme::SPACE_XS);
            }
        }
    }
    retained_notice(ui, view);
    view.pending_cursor = None;
}

pub(super) fn retained_notice(ui: &mut egui::Ui, view: &mut ViewState) {
    let mut clear = None;
    for (key, input) in &view.retained_inputs {
        ui.push_id(("retained-writing-input", key), |ui| {
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "保留输入 · {}:{} · 稿{}",
                    key.target.kind, key.target.id, key.attempt
                ))
                .strong(),
            );
            ui.add(egui::Label::new(theme::muted(key.path.to_string_lossy())).wrap());
            ui.colored_label(
                theme::ERROR(),
                format!(
                    "尚未插入这份正文：{}",
                    if input.error.is_empty() {
                        "组合输入已保留，请核对后继续"
                    } else {
                        &input.error
                    }
                ),
            );
            ui.add(egui::Label::new(&input.text).wrap());
            ui.horizontal_wrapped(|ui| {
                if ui.button("复制保留的输入").clicked() {
                    ui.ctx().copy_text(input.text.clone());
                }
                if crate::theme::add_enabled(
                    ui,
                    !view.ime_active,
                    egui::Button::new("清除保留输入"),
                )
                .clicked()
                {
                    view.retained_clear_confirm = Some(key.clone());
                }
            });
            ui.label(theme::muted(
                "先核对该来源的新内容，再在源码中合并；其他章节输入继续保留。",
            ));
            if view.retained_clear_confirm.as_ref() == Some(key) {
                ui.label("仅清除这份保留输入，其他章节和源文件草稿保持原样。");
                ui.horizontal_wrapped(|ui| {
                    if crate::theme::add_enabled(
                        ui,
                        !view.ime_active,
                        egui::Button::new("确认清除保留输入"),
                    )
                    .clicked()
                    {
                        clear = Some(key.clone());
                    }
                    if ui.button("继续保留输入").clicked() {
                        view.retained_clear_confirm = None;
                    }
                });
            }
        });
    }
    if let Some(key) = clear {
        view.retained_inputs.remove(&key);
        view.retained_clear_confirm = None;
    }
}

fn link_context_menu(response: &egui::Response, view: &ViewState, action: &mut Action) {
    response.context_menu(|ui| {
        if crate::theme::add_enabled(ui, !view.ime_active, egui::Button::new("关联世界资料…"))
            .clicked()
        {
            action.world_link = true;
            ui.close();
        }
    });
}
