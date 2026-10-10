use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn text(
    ui: &mut egui::Ui,
    form: &mut Form,
    id: egui::Id,
    value: &mut String,
    multiline: bool,
    typography: Typography,
    enabled: bool,
    receiver: Option<egui::Id>,
) -> bool {
    form.input_ids.insert(id);
    // egui 会在远处按下时先失焦；同帧已送往此字段的 Text/Paste 仍须先收齐。
    // 只暂存派生 pointer，不重放或过滤原始事件，其他控件仍正常产生 pending intent。
    let _batch = (form.batch_owner == Some(id)).then(|| {
        ui.ctx().memory_mut(|memory| memory.request_focus(id));
        BatchPointer {
            ctx: ui.ctx().clone(),
            pointer: ui
                .ctx()
                .input_mut(|input| std::mem::take(&mut input.pointer)),
        }
    });
    super::super::prepare_text_undo(ui.ctx(), id, value);
    let font = theme::body_font(typography.size);
    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, width: f32| {
        let mut job = egui::text::LayoutJob::simple(
            text.as_str().to_owned(),
            font.clone(),
            theme::TEXT(),
            width,
        );
        for section in &mut job.sections {
            section.format.line_height = Some(typography.size * typography.spacing);
        }
        ui.fonts(|fonts| fonts.layout_job(job))
    };
    let output = theme::add_enabled_ui(ui, enabled || receiver == Some(id), |ui| {
        // 仅此正式字段接收本轮 IME；有效投影与来源失效救援走同一路径。
        let _ime = super::ime::ReceiverScope::begin(ui, form, id);
        let editor = if multiline {
            egui::TextEdit::multiline(value)
                .desired_rows(2)
                .frame(false)
                .font(theme::body_font(typography.size))
                .layouter(&mut layouter)
        } else {
            egui::TextEdit::singleline(value)
        };
        editor.id(id).desired_width(f32::INFINITY).show(ui)
    })
    .inner;
    super::super::register_input(&output.response);
    preview_keyboard::text_control(ui, &output.response);
    form.drawn_inputs.push(id);
    if output.response.has_focus() {
        form.last_input = Some(id);
    }
    super::super::remember_text_undo(ui.ctx(), id, &output.galley.job.text);
    if form.focus && multiline && enabled && ui.is_enabled() {
        output.response.request_focus();
        form.focus = false;
    }
    output.response.changed()
}
pub(super) fn speaker(
    ui: &mut egui::Ui,
    value: &mut Option<TargetRef>,
    speakers: &[ReviewSpeaker],
    enabled: bool,
    action: &mut Action,
) {
    theme::add_enabled_ui(ui, enabled, |ui| {
        ui.horizontal_wrapped(|ui| {
            let selected = value
                .as_ref()
                .and_then(|target| speakers.iter().find(|speaker| &speaker.target == target))
                .map(|speaker| format!("{} · character:{}", speaker.display, speaker.target.id))
                .unwrap_or_else(|| "明确选择正式角色".into());
            let speaker = egui::ComboBox::from_id_salt("dialogue-speaker")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for speaker in speakers {
                        ui.selectable_value(
                            value,
                            Some(speaker.target.clone()),
                            format!("{} · character:{}", speaker.display, speaker.target.id),
                        );
                    }
                });
            preview_keyboard::control(ui, speaker.response);
            if let Some(target) = value {
                if preview_keyboard::add(ui, true, egui::Button::new("人物资料").small()).clicked()
                {
                    action.reference = Some(target.clone());
                }
            }
            if preview_keyboard::add(ui, true, egui::Button::new("新建人物…").small()).clicked()
            {
                action.create_character = true;
            }
        });
    });
}
#[allow(clippy::too_many_arguments)]
pub(super) fn draft(
    ui: &mut egui::Ui,
    form: &mut Form,
    draft: &mut DialogueDraft,
    key: &Key,
    typography: Typography,
    actions_enabled: bool,
    editable: bool,
    receiver: Option<egui::Id>,
    action: &mut Action,
) {
    if draft.kind == DialogueKind::Say {
        speaker(
            ui,
            &mut draft.speaker,
            &form.speakers,
            actions_enabled,
            action,
        );
    }
    if draft.parts.is_empty()
        && actions_enabled
        && preview_keyboard::button(ui, "添加文字").clicked()
    {
        draft.parts.push(DialoguePart::Literal {
            text: String::new(),
        });
    }
    let mut remove = None;
    for (index, part) in draft.parts.iter_mut().enumerate() {
        let id = egui::Id::new(("dialogue-part", key, index));
        ui.push_id(id, |ui| {
            match part {
                DialoguePart::Literal { text: value } => {
                    text(ui, form, id, value, true, typography, editable, receiver);
                }
                DialoguePart::Expression { source } => {
                    ui.label(theme::muted("正式表达式 · 未求值；修改后由 core 验证"));
                    text(ui, form, id, source, false, typography, editable, receiver);
                }
                DialoguePart::Link { target, label } => {
                    ui.label(theme::muted("强链接 · 明确目标与显示文字"));
                    text(
                        ui,
                        form,
                        id.with("kind"),
                        &mut target.kind,
                        false,
                        typography,
                        editable,
                        receiver,
                    );
                    text(
                        ui,
                        form,
                        id.with("id"),
                        &mut target.id,
                        false,
                        typography,
                        editable,
                        receiver,
                    );
                    text(ui, form, id, label, false, typography, editable, receiver);
                }
            }
            if draft.kind == DialogueKind::Say
                && preview_keyboard::add(ui, actions_enabled, egui::Button::new("移除此段").small())
                    .clicked()
            {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        draft.parts.remove(index);
    }
    theme::add_enabled_ui(ui, actions_enabled, |ui| {
        let menu = ui.menu_button("添加文字或正式 token", |ui| {
            if ui.button("字面文字").clicked() {
                draft.parts.push(DialoguePart::Literal {
                    text: String::new(),
                });
                ui.close();
            }
            if ui.button("表达式…").clicked() {
                draft.parts.push(DialoguePart::Expression {
                    source: String::new(),
                });
                ui.close();
            }
            if ui.button("强链接…").clicked() {
                draft.parts.push(DialoguePart::Link {
                    target: TargetRef::new("character", ""),
                    label: String::new(),
                });
                ui.close();
            }
        });
        preview_keyboard::control(ui, menu.response);
    });
    if draft.kind == DialogueKind::Say {
        let note = egui::CollapsingHeader::new("演出备注 · 作者私密")
            .default_open(draft.direction.is_some())
            .show(ui, |ui| {
                let mut present = draft.direction.is_some();
                if preview_keyboard::add(
                    ui,
                    actions_enabled,
                    egui::Checkbox::new(&mut present, "此句含演出备注"),
                )
                .changed()
                {
                    draft.direction = present.then(String::new);
                }
                if let Some(direction) = &mut draft.direction {
                    text(
                        ui,
                        form,
                        egui::Id::new(("dialogue-direction", key)),
                        direction,
                        true,
                        typography,
                        editable,
                        receiver,
                    );
                }
            });
        preview_keyboard::control(ui, note.header_response);
    }
    ui.label(theme::muted(
        "这些字段是尚未写入正文的输入；解码文字没有已证实的源码选区映射，选词关联请到源码。",
    ));
}

struct BatchPointer {
    ctx: egui::Context,
    pointer: egui::PointerState,
}
impl Drop for BatchPointer {
    fn drop(&mut self) {
        self.ctx
            .input_mut(|input| input.pointer = std::mem::take(&mut self.pointer));
    }
}
