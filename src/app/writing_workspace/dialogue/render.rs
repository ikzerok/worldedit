use super::*;

pub(super) fn draw(
    ui: &mut egui::Ui,
    project: &Project,
    buffer: &WritingBuffer,
    view: &mut ViewState,
    typography: Typography,
    action: &mut Action,
    projection: &DialogueProjection,
) {
    let key = Key::new(buffer, &projection.target);
    view.dialogue.begin_render(&key);
    focus::prepare(ui, buffer, view, projection, &key);
    let indices = view.dialogue.statement_indices.clone();
    render_state::pages(ui, &mut view.dialogue, projection, !view.ime_active, false);
    let mut form_drawn = false;
    ui.label(theme::muted("正式对白 · Enter 在本句换行；Ctrl/⌘+Enter 预览本句后续写下一句，未修改的语句直接开启下一句。"));
    if view.dialogue.insert_requested {
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt(("dialogue-insert-anchor", &key))
                .selected_text(
                    view.dialogue
                        .anchor
                        .as_ref()
                        .and_then(|id| projection.anchors.iter().find(|anchor| &anchor.id == id))
                        .map(|anchor| anchor.label.as_str())
                        .unwrap_or("明确选择插入位置"),
                )
                .show_ui(ui, |ui| {
                    for anchor in &projection.anchors {
                        ui.selectable_value(
                            &mut view.dialogue.anchor,
                            Some(anchor.id.clone()),
                            format!("第{}行 · {}", anchor.line, anchor.label),
                        );
                    }
                });
            let anchor = view
                .dialogue
                .anchor
                .as_ref()
                .and_then(|id| projection.anchors.iter().find(|anchor| &anchor.id == id));
            if theme::add_enabled(
                ui,
                anchor.is_some()
                    && !view.ime_active
                    && !view.dialogue.forms.get(&key).is_some_and(Form::protected),
                egui::Button::new("在此写正式台词"),
            )
            .clicked()
            {
                view.dialogue.begin(
                    ui,
                    buffer,
                    projection,
                    DialogueOperation::Insert {
                        anchor_id: anchor.unwrap().id.clone(),
                        draft: DialogueDraft {
                            kind: DialogueKind::Say,
                            speaker: None,
                            direction: None,
                            parts: vec![DialoguePart::Literal {
                                text: String::new(),
                            }],
                        },
                    },
                    "新正式台词".into(),
                );
            }
            if ui.button("取消插入位置").clicked() {
                view.dialogue.insert_requested = false;
            }
        });
        if projection.anchors.is_empty() {
            ui.label(theme::muted(
                "当前来源没有 core 可证明的插入位置，请在结构或源码中继续。",
            ));
        }
    }
    if view
        .dialogue
        .forms
        .get(&key)
        .is_some_and(|form| matches!(form.request.operation, DialogueOperation::Insert { .. }))
    {
        form::draw(
            ui, project, buffer, view, typography, action, projection, &key,
        );
        form_drawn = true;
    }
    let range = view.dialogue.row_offset
        ..(view.dialogue.row_offset + render_state::PAGE_ROWS).min(projection.rows.len());
    let active_in_page = view.dialogue.forms.get(&key).is_some_and(|form| {
        let id = match &form.request.operation {
            DialogueOperation::Update { statement_id, .. }
            | DialogueOperation::Convert { statement_id, .. }
            | DialogueOperation::Delete { statement_id } => Some(statement_id),
            DialogueOperation::Insert { .. } => None,
        };
        id.is_some_and(|id| {
            projection.rows[range.clone()]
                .iter()
                .any(|row| row.statement_id.as_ref() == Some(id))
        })
    });
    if !form_drawn && !active_in_page && view.dialogue.forms.contains_key(&key) {
        ui.colored_label(
            theme::WARNING(),
            "保留的语句输入不在当前正文段；可在此继续编辑、核对或明确取消。",
        );
        form::draw(
            ui, project, buffer, view, typography, action, projection, &key,
        );
        form_drawn = true;
    }
    for row in &projection.rows[range] {
        #[cfg(test)]
        {
            view.dialogue.rendered_rows += 1;
        }
        ui.push_id(
            (
                &key,
                row.source.as_ref().map(|source| source.byte_start),
                &row.statement_id,
                &row.label,
            ),
            |ui| {
                let statement = row
                    .statement_id
                    .as_ref()
                    .and_then(|id| indices.get(id))
                    .and_then(|index| projection.statements.get(*index));
                if let Some(statement) = statement {
                    let active = view.dialogue.forms.get(&key).is_some_and(|form| {
                        match &form.request.operation {
                            DialogueOperation::Update { statement_id, .. }
                            | DialogueOperation::Convert { statement_id, .. }
                            | DialogueOperation::Delete { statement_id } => {
                                statement_id == &statement.id
                            }
                            _ => false,
                        }
                    });
                    if active {
                        form_drawn = true;
                        form::draw(
                            ui, project, buffer, view, typography, action, projection, &key,
                        );
                    } else {
                        row_body(
                            ui,
                            statement,
                            projection,
                            typography,
                            action,
                            !view.ime_active,
                        );
                        let available = !view.ime_active
                            && !view.dialogue.forms.get(&key).is_some_and(Form::protected);
                        ui.horizontal_wrapped(|ui| {
                            if theme::add_enabled(
                                ui,
                                available,
                                egui::Button::new("编辑此句").small(),
                            )
                            .clicked()
                            {
                                view.dialogue.begin(
                                    ui,
                                    buffer,
                                    projection,
                                    DialogueOperation::Update {
                                        statement_id: statement.id.clone(),
                                        draft: statement.draft.clone(),
                                    },
                                    "编辑正式语句".into(),
                                );
                            }
                            if theme::add_enabled(
                                ui,
                                available && statement.after_anchor_id.is_some(),
                                egui::Button::new("下一句").small(),
                            )
                            .clicked()
                            {
                                next(ui, buffer, &mut view.dialogue, projection, statement);
                            }
                            ui.menu_button("语句操作", |ui| {
                                if theme::add_enabled(
                                    ui,
                                    available,
                                    egui::Button::new(if statement.kind == DialogueKind::Text {
                                        "转为正式台词…"
                                    } else {
                                        "转为普通旁白…"
                                    }),
                                )
                                .clicked()
                                {
                                    view.dialogue.begin(
                                        ui,
                                        buffer,
                                        projection,
                                        DialogueOperation::Convert {
                                            statement_id: statement.id.clone(),
                                            to: if statement.kind == DialogueKind::Text {
                                                DialogueKind::Say
                                            } else {
                                                DialogueKind::Text
                                            },
                                            speaker: None,
                                            allow_direction_loss: false,
                                        },
                                        "转换正式单语句".into(),
                                    );
                                    ui.close();
                                }
                                if theme::add_enabled(
                                    ui,
                                    available,
                                    egui::Button::new("删除此语句…"),
                                )
                                .clicked()
                                {
                                    view.dialogue.begin(
                                        ui,
                                        buffer,
                                        projection,
                                        DialogueOperation::Delete {
                                            statement_id: statement.id.clone(),
                                        },
                                        "删除正式语句".into(),
                                    );
                                    ui.close();
                                }
                                if theme::add_enabled(
                                    ui,
                                    !view.ime_active,
                                    egui::Button::new("定位完整源码语句"),
                                )
                                .clicked()
                                {
                                    view.restore_mode(Mode::Source);
                                    crate::app::search::request_writing_selection(
                                        ui.ctx(),
                                        buffer.path().into(),
                                        buffer.source().into(),
                                        statement.source.byte_start..statement.source.byte_end,
                                        buffer.generation(),
                                    );
                                    ui.close();
                                }
                            });
                        });
                    }
                } else {
                    let indent = row.depth.min(5) as f32 * 8.0;
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(indent);
                        ui.add(egui::Label::new(theme::muted(&row.label)).wrap());
                    });
                }
                ui.add_space(typography.size * 0.35);
            },
        );
    }
    if !form_drawn && view.dialogue.forms.contains_key(&key) {
        ui.colored_label(
            theme::WARNING(),
            "保留的语句输入不在当前正文段；仍可在这里继续编辑、核对或明确取消。",
        );
        form::draw(
            ui, project, buffer, view, typography, action, projection, &key,
        );
    }
    render_state::pages(ui, &mut view.dialogue, projection, !view.ime_active, true);
    // 包括当前页之外的旧表单也已消费本帧输入，才可以替换。
    view.dialogue.finish_pending(ui.ctx(), buffer, projection);
}
fn row_body(
    ui: &mut egui::Ui,
    statement: &DialogueStatement,
    projection: &DialogueProjection,
    typography: Typography,
    action: &mut Action,
    enabled: bool,
) {
    if let Some(target) = &statement.draft.speaker {
        let display = projection
            .speakers
            .iter()
            .find(|speaker| &speaker.target == target)
            .map(|speaker| speaker.display.as_str())
            .unwrap_or(&target.id);
        let response = theme::add_enabled(
            ui,
            enabled,
            egui::Button::new(egui::RichText::new(display).strong()).frame(false),
        )
        .on_hover_text(format!("{}:{} · 查看人物资料", target.kind, target.id));
        super::super::preview_navigation::reveal(ui, &response, true, true);
        if response.clicked() {
            action.reference = Some(target.clone());
        }
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for part in &statement.draft.parts {
            match part {
                DialoguePart::Literal { text } => {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(text).font(theme::body_font(typography.size)),
                        )
                        .wrap(),
                    );
                }
                DialoguePart::Expression { source } => {
                    ui.label(
                        egui::RichText::new(format!("⟦{source}⟧"))
                            .font(theme::body_font(typography.size))
                            .color(theme::BLUE()),
                    )
                    .on_hover_text("正式表达式 · 未求值");
                }
                DialoguePart::Link { target, label } => {
                    let response = theme::add_enabled(
                        ui,
                        enabled,
                        egui::Button::new(
                            egui::RichText::new(label)
                                .font(theme::body_font(typography.size))
                                .color(theme::BLUE()),
                        )
                        .frame(false),
                    );
                    super::super::preview_navigation::reveal(ui, &response, true, true);
                    if response.clicked() {
                        action.reference = Some(target.clone());
                    }
                }
            }
        }
        if statement.draft.parts.is_empty() {
            ui.label(theme::muted("空语句"));
        }
    });
    if let Some(direction) = &statement.draft.direction {
        egui::CollapsingHeader::new("演出备注 · 作者私密").show(ui, |ui| {
            ui.add(egui::Label::new(theme::muted(direction)).wrap());
        });
    }
}
pub(super) fn next(
    ui: &egui::Ui,
    buffer: &WritingBuffer,
    state: &mut State,
    projection: &DialogueProjection,
    statement: &DialogueStatement,
) {
    if let Some(anchor_id) = &statement.after_anchor_id {
        state.begin(
            ui,
            buffer,
            projection,
            DialogueOperation::Insert {
                anchor_id: anchor_id.clone(),
                draft: DialogueDraft {
                    kind: DialogueKind::Say,
                    speaker: statement.draft.speaker.clone(),
                    direction: None,
                    parts: vec![DialoguePart::Literal {
                        text: String::new(),
                    }],
                },
            },
            "下一句 · 角色可修改，不自动轮换".into(),
        );
    }
}
