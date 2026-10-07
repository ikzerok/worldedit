//! 正文、结构与源码共用 core 按文件唯一的 WritingBuffer。
mod composition_text;
mod editors;
mod input;
mod input_registry;
pub(in crate::app) use input_registry::register_input;
mod prose;
mod session;
mod text_undo;
use crate::theme;
pub(in crate::app) use session::fingerprint;
pub(super) use session::WritingCursor;
pub(in crate::app) use text_undo::{prepare_text_undo, remember_text_undo};
use worldline_core::catalog::TargetRef;
use worldline_core::manuscript::WritingBuffer;
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
    selection_mode: Option<Mode>,
    pending_cursor: Option<WritingCursor>,
    pending_focus: bool,
    ime_active: bool,
    composition: input::Composition,
    raw_blocked_buttons: u8,
    retained_inputs: std::collections::BTreeMap<prose::RetainedKey, prose::RetainedInput>,
    composing_inputs:
        std::collections::BTreeMap<prose::RetainedKey, composition_text::PendingInput>,
    retained_clear_confirm: Option<prose::RetainedKey>,
}

#[derive(Default)]
pub(super) struct Action {
    pub apply: bool,
    pub comment: bool,
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
    pub source_size: f32,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_controls(
    ui: &mut egui::Ui,
    project: &Project,
    buffer: &mut WritingBuffer,
    target: &TargetRef,
    title: &str,
    view: &mut ViewState,
    typography: Typography,
) -> Action {
    view.prepare_restore(buffer, target);
    let input_busy = view.ime_active;
    let mut action = Action::default();
    let previous_mode = view.mode;
    let narrow = ui.ctx().screen_rect().width() < 600.0 || ui.ctx().screen_rect().height() < 420.0;
    let compact_title = typography.compact && theme::style_preset() != theme::StylePreset::Ledger;
    ui.horizontal_wrapped(|ui| {
        let identity = format!("{}:{}", target.kind, target.id);
        let heading = if compact_title {
            let font = egui::TextStyle::Heading.resolve(ui.style());
            let natural = ui.fonts(|fonts| {
                fonts
                    .layout_no_wrap(title.to_owned(), font, theme::TEXT())
                    .size()
                    .x
            });
            let width = natural.min((ui.available_width() * 0.25).clamp(72.0, 180.0));
            ui.add_sized(
                [width, ui.spacing().interact_size.y],
                egui::Label::new(egui::RichText::new(title).heading().strong()).truncate(),
            )
        } else {
            ui.heading("正文")
        };
        heading
            .on_hover_text(format!("{title}\n{identity}"))
            .context_menu(|ui| {
                if ui.button("复制来源对象身份").clicked() {
                    ui.ctx().copy_text(identity);
                    ui.close();
                }
            });
        if narrow {
            // This menu includes a two-step discard confirmation. egui menus
            // otherwise close on every click, hiding the second step immediately.
            egui::containers::menu::MenuButton::new("正文工具")
                .config(
                    egui::containers::menu::MenuConfig::default()
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                )
                .ui(ui, |ui| {
                    let discard_was_pending = view.discard_confirm.is_some();
                    ui.set_max_width((ui.ctx().screen_rect().width() - 32.0).min(360.0));
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .show(ui, |ui| {
                            draw_actions(ui, buffer, view, input_busy, &mut action);
                            draw_status(ui, project, buffer, view, input_busy, &mut action, true);
                        });
                    if previous_mode != view.mode
                        || action.apply
                        || action.discard
                        || action.comment
                        || (discard_was_pending && view.discard_confirm.is_none())
                    {
                        ui.close();
                    }
                });
            ui.label(theme::muted(if buffer.is_changed() {
                "未应用"
            } else {
                "已应用"
            }));
        } else {
            draw_actions(ui, buffer, view, input_busy, &mut action);
        }
    });
    if previous_mode != view.mode {
        view.selection_mode = None;
    }
    if !narrow {
        draw_status(
            ui,
            project,
            buffer,
            view,
            input_busy,
            &mut action,
            !typography.compact,
        );
    }
    action
}

fn draw_actions(
    ui: &mut egui::Ui,
    buffer: &WritingBuffer,
    view: &mut ViewState,
    input_busy: bool,
    action: &mut Action,
) {
    crate::theme::add_enabled_ui(ui, !input_busy, |ui| {
        ui.selectable_value(&mut view.mode, Mode::Prose, "写作");
        ui.selectable_value(&mut view.mode, Mode::Structure, "结构");
        ui.selectable_value(&mut view.mode, Mode::Source, "源码");
    });
    if crate::theme::add_enabled(
        ui,
        buffer.is_changed() && !input_busy && !view.has_retained_for(buffer.path()),
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
    if crate::theme::add_enabled(
        ui,
        buffer.is_changed() && !input_busy,
        egui::Button::new("丢弃此文件草稿"),
    )
    .clicked()
    {
        view.discard_confirm = Some(buffer.path().to_owned());
    }
}

fn draw_status(
    ui: &mut egui::Ui,
    project: &Project,
    buffer: &WritingBuffer,
    view: &mut ViewState,
    input_busy: bool,
    action: &mut Action,
    source_caption: bool,
) {
    if input_busy {
        ui.label(theme::muted(
            "输入法组合中 · 请完成输入后再切换、应用或返回",
        ));
    }
    if source_caption {
        theme::source_caption(ui, &project.root, buffer.path());
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(theme::muted(if buffer.is_changed() {
            "未应用草稿 · 尚未保存"
        } else {
            "正文与当前工程一致"
        }));
        if ui
            .small_button("批注选区")
            .on_hover_text("为当前选区添加批注：先预览完整源码行，不自动应用或保存")
            .clicked()
        {
            action.comment = true;
        }
    });
    if view.discard_confirm.as_deref() == Some(buffer.path()) {
        ui.colored_label(
            theme::ERROR(),
            "将丢弃此源文件所有章节的未应用输入，工程原文不变。",
        );
        ui.horizontal_wrapped(|ui| {
            if crate::theme::add_enabled(ui, !input_busy, egui::Button::new("确认丢弃正文草稿"))
                .clicked()
            {
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
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_document(
    ui: &mut egui::Ui,
    project: &Project,
    buffer: &mut WritingBuffer,
    target: &TargetRef,
    title: &str,
    view: &mut ViewState,
    typography: Typography,
    action: &mut Action,
) {
    let width = if view.mode == Mode::Prose {
        typography.width
    } else {
        ui.available_width()
    };
    theme::document_surface_titled(ui, title, width, |ui| {
        // max_rect follows the content origin, whereas clip_rect stays on screen.
        // A saved offset must never add the same distance to the page's height.
        ui.set_min_height(ui.available_height());
        if !typography.compact && theme::style_preset() != theme::StylePreset::Ledger {
            theme::panel_header(ui, title, "");
        }
        editors::draw(ui, project, buffer, target, view, typography, action);
    });
}
