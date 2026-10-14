//! 正文、结构与源码共用 core 按文件唯一的 WritingBuffer。
mod composition_text;
mod context_selection;
mod dialogue;
mod editors;
mod input;
mod input_registry;
mod modes;
pub(in crate::app) mod preview_navigation;
pub(in crate::app) use input_registry::register_input;
mod projection_cache;
mod prose;
mod session;
mod text_undo;
mod toolbar_layout;
mod toolbar_menu;
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
    projection_cache: projection_cache::ProjectionCache,
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
    dialogue: dialogue::State,
    mode_request: Option<modes::Request>,
    prose_return_cursor: Option<WritingCursor>,
    frame_input_owner: Option<(u64, Option<egui::Id>)>,
}

#[derive(Default)]
pub(super) struct Action {
    pub apply: bool,
    pub comment: bool,
    pub world_link: bool,
    pub source_mode: bool,
    pub discard: bool,
    pub error: Option<String>,
    pub dialogue_plan: Option<worldline_core::manuscript::DialogueEditPlan>,
    pub dialogue_continue: bool,
    pub reference: Option<TargetRef>,
    pub production: bool,
    pub create_character: bool,
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
    let compact_title = typography.compact && theme::style_preset() != theme::StylePreset::Ledger;
    let heading_width = toolbar_layout::heading_width(ui, title, compact_title);
    let layout = toolbar_layout::choose(ui, heading_width, view.mode);
    let narrow = matches!(
        layout,
        toolbar_layout::Layout::Modes | toolbar_layout::Layout::Compact
    );
    ui.horizontal_wrapped(|ui| {
        let identity = format!("{}:{}", target.kind, target.id);
        let heading = if compact_title {
            ui.add_sized(
                [heading_width, ui.spacing().interact_size.y],
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
            if layout == toolbar_layout::Layout::Modes {
                draw_modes(ui, buffer, target, view, input_busy);
            }
            toolbar_menu::draw(
                ui,
                project,
                buffer,
                target,
                view,
                input_busy,
                &mut action,
                layout == toolbar_layout::Layout::Compact,
            );
            if layout == toolbar_layout::Layout::Compact {
                ui.label(theme::muted(if buffer.is_changed() {
                    "未应用"
                } else {
                    "已应用"
                }));
            }
        } else {
            draw_modes(ui, buffer, target, view, input_busy);
            draw_actions(ui, buffer, target, view, input_busy, &mut action);
            if layout == toolbar_layout::Layout::Full {
                dialogue::toolbar(ui, buffer, target, view, &mut action, input_busy);
            } else {
                dialogue::toolbar_menu(ui, buffer, target, view, &mut action, input_busy);
            }
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
            target,
            view,
            input_busy,
            &mut action,
            !typography.compact,
        );
    }
    action
}

fn draw_modes(
    ui: &mut egui::Ui,
    buffer: &WritingBuffer,
    target: &TargetRef,
    view: &mut ViewState,
    input_busy: bool,
) {
    crate::theme::add_enabled_ui(ui, !input_busy, |ui| {
        for (mode, label) in [
            (Mode::Prose, "写作"),
            (Mode::Structure, "结构"),
            (Mode::Source, "源码"),
        ] {
            let response = ui.selectable_label(view.mode == mode, label);
            view.protect_toolbar_input(ui, &response, buffer, target);
            if response.clicked() {
                view.request_mode(ui.ctx(), buffer, target, mode);
            }
        }
    });
}

fn draw_actions(
    ui: &mut egui::Ui,
    buffer: &WritingBuffer,
    target: &TargetRef,
    view: &mut ViewState,
    input_busy: bool,
    action: &mut Action,
) {
    let apply = crate::theme::add_enabled(
        ui,
        buffer.is_changed() && !input_busy && !view.has_retained_for(buffer.path()),
        egui::Button::new(toolbar_layout::apply_label(view.mode)),
    );
    view.protect_toolbar_input(ui, &apply, buffer, target);
    if apply.clicked() {
        action.apply = true;
        action.source_mode = view.mode == Mode::Source;
    }
    let discard = crate::theme::add_enabled(
        ui,
        (buffer.is_changed() || view.has_retained_for(buffer.path())) && !input_busy,
        egui::Button::new("丢弃此文件草稿"),
    );
    view.protect_toolbar_input(ui, &discard, buffer, target);
    if discard.clicked() {
        view.discard_confirm = Some(buffer.path().to_owned());
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_status(
    ui: &mut egui::Ui,
    project: &Project,
    buffer: &mut WritingBuffer,
    target: &TargetRef,
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
        ui.menu_button("选词工具", |ui| {
            if crate::theme::add_enabled(
                ui,
                !input_busy && !view.dialogue_selection_focused(ui.ctx()),
                egui::Button::new("关联世界资料…"),
            )
            .on_hover_text("选择正文文字，检索已有资料或创建正式人物 / 实体并关联")
            .clicked()
            {
                action.world_link = true;
                ui.close();
            }
        });
        if crate::theme::add_enabled(
            ui,
            !input_busy && !view.dialogue_selection_focused(ui.ctx()),
            egui::Button::new("批注选区").small(),
        )
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
            let confirm =
                crate::theme::add_enabled(ui, !input_busy, egui::Button::new("确认丢弃正文草稿"));
            view.protect_toolbar_input(ui, &confirm, buffer, target);
            if confirm.clicked() {
                action.discard = true;
                view.discard_confirm = None;
            }
            let cancel = ui.button("取消丢弃");
            view.protect_toolbar_input(ui, &cancel, buffer, target);
            if cancel.clicked() {
                view.discard_confirm = None;
            }
        });
    }
    if buffer.baseline() != project.content_baseline() {
        ui.colored_label(
            theme::ERROR(),
            "工程基线已变化；草稿完整保留，不能覆盖新内容。",
        );
        if !buffer.is_changed() && view.dialogue_retained_for(buffer.path()) {
            let reload = crate::theme::add_enabled(
                ui,
                !input_busy,
                egui::Button::new("载入已应用原文，保留对白字段"),
            );
            view.protect_toolbar_input(ui, &reload, buffer, target);
            if reload.clicked() {
                view.request_reload(ui.ctx(), buffer, target);
            }
        }
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
    let input_scope = view.mode_input_scope(ui, buffer, target);
    // Only geometry: the ordinary body gains no focus stop or key handler.
    view.dialogue.viewport = Some(
        egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), ui.clip_rect().y_range())
            .intersect(ui.clip_rect()),
    );
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
    drop(input_scope);
    if ui.is_enabled() {
        view.finish_toolbar_request(ui.ctx(), project, buffer, target, action);
    } else {
        view.mode_request = None;
    }
}
