//! 正文、结构与源码共用 core 按文件唯一的 WritingBuffer。
mod session;
mod text_undo;
use crate::theme;
pub(super) use session::WritingCursor;
pub(in crate::app) use text_undo::{prepare_text_undo, remember_text_undo};
use worldline_core::catalog::TargetRef;
use worldline_core::manuscript::{WritingBlockKind, WritingBuffer};
use worldline_core::project::Project;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::app) enum Mode {
    Structure,
    Source,
    #[default]
    #[serde(other)]
    Prose,
}

impl Mode {
    fn key(self) -> &'static str {
        match self {
            Self::Prose => "prose",
            Self::Structure => "structure",
            Self::Source => "source",
        }
    }
}

#[derive(Default)]
pub(super) struct ViewState {
    mode: Mode,
    discard_confirm: Option<std::path::PathBuf>,
    cursor: Option<WritingCursor>,
    pending_cursor: Option<WritingCursor>,
}

#[derive(Default)]
pub(super) struct Action {
    pub apply: bool,
    pub source_mode: bool,
    pub discard: bool,
    pub error: Option<String>,
}

#[derive(Clone, Copy)]
pub(super) struct Typography {
    pub compact: bool,
    pub size: f32,
    pub spacing: f32,
    pub width: f32,
}

pub(super) fn draw(
    ui: &mut egui::Ui,
    project: &Project,
    buffer: &mut WritingBuffer,
    target: &TargetRef,
    view: &mut ViewState,
    typography: Typography,
) -> Action {
    view.prepare_restore(buffer, target);
    let mut action = Action::default();
    ui.horizontal_wrapped(|ui| {
        ui.heading("正文");
        ui.selectable_value(&mut view.mode, Mode::Prose, "写作");
        ui.selectable_value(&mut view.mode, Mode::Structure, "结构");
        ui.selectable_value(&mut view.mode, Mode::Source, "源码");
        if ui
            .add_enabled(
                buffer.is_changed(),
                egui::Button::new(if view.mode == Mode::Source {
                    "应用源码草稿（可含诊断）"
                } else {
                    "应用正文草稿"
                }),
            )
            .clicked()
        {
            action.apply = true;
            action.source_mode = view.mode == Mode::Source;
        }
        if ui
            .add_enabled(buffer.is_changed(), egui::Button::new("丢弃此文件草稿"))
            .clicked()
        {
            view.discard_confirm = Some(buffer.path().to_owned());
        }
    });
    if !typography.compact {
        let source_label = format!(
            "{}:{} · {}",
            target.kind,
            target.id,
            buffer.path().display()
        );
        ui.add(egui::Label::new(theme::muted(&source_label)).truncate())
            .on_hover_text(source_label);
    }
    ui.label(theme::muted(if buffer.is_changed() {
        "未应用草稿 · 尚未保存"
    } else {
        "正文与当前工程一致"
    }));
    if view.discard_confirm.as_deref() == Some(buffer.path()) {
        ui.colored_label(
            theme::ERROR(),
            "将丢弃此源文件所有章节的未应用输入，工程原文不变。",
        );
        ui.horizontal(|ui| {
            if ui.button("确认丢弃正文草稿").clicked() {
                action.discard = true;
                view.discard_confirm = None;
            }
            if ui.button("取消丢弃").clicked() {
                view.discard_confirm = None;
            }
        });
    }
    if buffer.baseline() != project.content_baseline() {
        ui.colored_label(
            theme::ERROR(),
            "工程基线已变化；草稿完整保留，不能覆盖新内容。",
        );
    }
    if view.mode == Mode::Source {
        if !typography.compact {
            ui.label(theme::muted(
                "完整源码；与写作和结构视图共用一个文件草稿。无效输入不会丢失。",
            ));
        }
        let mut source = buffer.source().to_owned();
        let id = egui::Id::new(("writing-source", buffer.path(), &target.kind, &target.id));
        view.restore_editor(ui, id, buffer, 0, &source);
        let output = egui::TextEdit::multiline(&mut source)
            .id(id)
            .code_editor()
            .desired_width(f32::INFINITY)
            .desired_rows(18)
            .show(ui);
        if output.response.changed() {
            buffer.replace_source(source.clone());
        }
        view.record_cursor(ui, &output, buffer, target, 0, &source);
        view.pending_cursor = None;
        return action;
    }
    let projection = match project.project_writing_buffer(buffer, target) {
        Ok(projection) => projection,
        Err(error) => {
            view.pending_cursor = None;
            ui.colored_label(theme::ERROR(), error);
            if ui.button("在源码视图继续编辑").clicked() {
                view.mode = Mode::Source;
            }
            return action;
        }
    };
    if view.mode == Mode::Structure {
        if !typography.compact {
            ui.label(theme::muted(
                "仅当前目标声明体；保留缩进、注释与所有复杂控制语句。",
            ));
        }
        let mut source = projection.source.clone();
        let offset = projection.range.start;
        let id = egui::Id::new(("writing-structure", buffer.path(), &target.kind, &target.id));
        view.restore_editor(ui, id, buffer, offset, &source);
        let output = egui::TextEdit::multiline(&mut source)
            .id(id)
            .code_editor()
            .desired_width(f32::INFINITY)
            .desired_rows(18)
            .show(ui);
        if output.response.changed() {
            action.error = buffer
                .replace_range(
                    projection.generation,
                    projection.range,
                    &projection.source,
                    &source,
                )
                .err();
        }
        view.record_cursor(ui, &output, buffer, target, offset, &source);
        view.pending_cursor = None;
        return action;
    }
    if !typography.compact {
        ui.label(theme::muted(
            "正文块可直接修改；内插、链接、转义和行标记保留为源文。结构不执行，可切换结构视图。",
        ));
    }
    ui.set_max_width(typography.width.min(ui.available_width()).max(120.0));
    if projection.blocks.is_empty() {
        ui.label("此来源还没有正文，请在结构视图开始写作。");
    }
    for block in &projection.blocks {
        match block.kind {
            WritingBlockKind::Prose => {
                let mut text = block.text.clone();
                let font = egui::FontId::proportional(typography.size);
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
                let id = egui::Id::new((
                    "writing-prose",
                    buffer.path(),
                    &target.kind,
                    &target.id,
                    block.range.start,
                ));
                view.restore_editor(ui, id, buffer, block.range.start, &text);
                let output = egui::TextEdit::multiline(&mut text)
                    .id(id)
                    .font(font.clone())
                    .layouter(&mut layouter)
                    .desired_width(f32::INFINITY)
                    .desired_rows(2)
                    .show(ui);
                if output.response.changed() {
                    action.error = buffer
                        .replace_prose(projection.generation, block, &text)
                        .err();
                }
                view.record_cursor(ui, &output, buffer, target, block.range.start, &text);
                if output.response.changed() {
                    break;
                }
            }
            WritingBlockKind::Structure if !block.text.is_empty() => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(theme::muted(&block.label));
                    ui.monospace(&block.text);
                });
            }
            _ => {
                ui.add_space(4.0);
            }
        }
    }
    view.pending_cursor = None;
    action
}
