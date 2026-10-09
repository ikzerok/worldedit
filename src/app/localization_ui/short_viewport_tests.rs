//! Full application frames at a fixed physical size; native desktop validation is separate.
use super::workbench_tests::fixture;
use super::*;
use crate::app::{Tab, WorldeditApp};
use egui::{Event, Pos2, Rect, Vec2};
use worldline_core::localization::LocalizationStatus;

pub(super) struct Harness {
    pub(super) ctx: egui::Context,
    pub(super) app: WorldeditApp,
    physical: Vec2,
    viewport: Rect,
    time: f64,
    scroll_delta: Vec2,
    pub(super) frame_seconds: f64,
    pub(super) trace_frames: bool,
    pub(super) frame_trace: std::collections::VecDeque<String>,
}

impl Harness {
    pub(super) fn new(source: &str) -> Self {
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = WorldeditApp::new(&creation, None);
        let (mut project, state) = fixture(1);
        project
            .set_text(&project.entry.clone(), source.into())
            .unwrap();
        project.save().unwrap();
        app.project = project;
        app.active_file = app.project.entry.clone();
        app.reset_views();
        app.recompile();
        app.saved_location = true;
        app.localization_ui = state;
        app.tab = Tab::Localization;
        app.personal.settings.appearance.ui_scale = 2.0;
        app.personal.settings.appearance.reduce_motion = true;
        let mut h = Self {
            ctx,
            app,
            physical: egui::vec2(763.0, 541.0),
            viewport: Rect::NOTHING,
            time: 0.0,
            scroll_delta: Vec2::ZERO,
            frame_seconds: 1.0 / 60.0,
            trace_frames: false,
            frame_trace: std::collections::VecDeque::new(),
        };
        for _ in 0..4 {
            h.frame(vec![]);
        }
        h.settle();
        h.assert_screen();
        h
    }

    pub(super) fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.time += self.frame_seconds;
        let trace = self.trace_frames.then(|| format!("{events:?}"));
        // eframe converts the fixed native pixel extent using the current effective scale.
        let screen = Rect::from_min_size(Pos2::ZERO, self.physical / self.ctx.pixels_per_point());
        let mut raw = egui::RawInput {
            screen_rect: Some(screen),
            time: Some(self.time),
            modifiers: events
                .iter()
                .find_map(|event| match event {
                    Event::Key { modifiers, .. } => Some(*modifiers),
                    _ => None,
                })
                .unwrap_or(egui::Modifiers::NONE),
            events,
            ..Default::default()
        };
        let viewport = raw.viewports.entry(egui::ViewportId::ROOT).or_default();
        viewport.native_pixels_per_point = Some(1.0);
        viewport.inner_rect = Some(screen);
        viewport.outer_rect = Some(screen);
        viewport.focused = Some(true);
        eframe::App::raw_input_hook(&mut self.app, &self.ctx, &mut raw);
        let mut scroll_delta = Vec2::ZERO;
        let output = self.ctx.run(raw, |ctx| {
            scroll_delta = ctx.input(|input| input.smooth_scroll_delta);
            eframe::App::update(&mut self.app, ctx, &mut eframe::Frame::_new_kittest());
        });
        self.scroll_delta = scroll_delta;
        if let Some(events) = trace {
            self.record_trace(super::keyboard_visibility_tests::describe_frame(
                self, &events,
            ));
        }
        output
    }

    pub(super) fn record_trace(&mut self, frame: String) {
        self.frame_trace.push_back(frame);
        while self.frame_trace.len() > 12 {
            self.frame_trace.pop_front();
        }
    }

    pub(super) fn settle(&mut self) {
        self.frame(vec![]);
        jobs::settle(
            &self.app.project,
            &mut self.app.localization_ui,
            self.app.version,
        );
        self.frame(vec![]);
    }

    fn assert_screen(&mut self) {
        let output = self.frame(vec![]);
        assert_eq!(self.ctx.zoom_factor(), 2.0);
        assert_eq!(self.ctx.native_pixels_per_point(), Some(1.0));
        assert_eq!(output.pixels_per_point, 2.0);
        assert_eq!(self.ctx.screen_rect().size(), egui::vec2(381.5, 270.5));
        assert_eq!(
            self.ctx.screen_rect().size() * output.pixels_per_point,
            self.physical
        );
        let (_, clip) = painted(&output, "本地化工作台").unwrap();
        assert!(
            clip.top() > 80.0 && clip.height() < 150.0,
            "global chrome missing: {clip:?}"
        );
        self.viewport = clip;
        self.visible("保存全部");
    }

    pub(super) fn visible(&mut self, label: &str) -> Pos2 {
        let mut last = String::new();
        let mut scanned_from_top = false;
        let mut stable: Option<Rect> = None;
        for _ in 0..100 {
            let output = self.frame(vec![]);
            let Some((rect, clip)) = painted(&output, label) else {
                stable = None;
                last = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) => Some(text.galley.text()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                // Fully clipped widgets may emit no paint; scan the real scroll area from its top.
                let delta = if scanned_from_top { -38.0 } else { 100_000.0 };
                scanned_from_top = true;
                let point = self.scroll_point();
                self.wheel(point, delta);
                continue;
            };
            let visible = rect.intersect(clip).intersect(self.ctx.screen_rect());
            if visible.is_positive()
                && visible.height() + 0.5 >= rect.height().min(clip.height())
                && visible.width() + 0.5 >= rect.width().min(clip.width())
            {
                if stable.is_some_and(|old| {
                    old.min.distance(rect.min) < 0.25 && old.max.distance(rect.max) < 0.25
                }) {
                    return visible.center();
                }
                stable = Some(rect);
                continue;
            }
            stable = None;
            last = format!("{label}: text={rect:?}, clip={clip:?}");
            let delta = if rect.top() < clip.top() {
                clip.top() - rect.top()
            } else {
                clip.bottom() - rect.bottom()
            };
            self.wheel(self.scroll_point(), delta.clamp(-38.0, 38.0));
        }
        panic!("763×541 physical / 200% cannot reach {label}: {last}; viewport={:?}, remaining scroll={:?}", self.viewport, self.scroll_delta);
    }

    pub(super) fn scroll_point(&self) -> Pos2 {
        if self.app.capability_ui.is_some() {
            self.ctx
                .memory(|memory| memory.area_rect(egui::Id::new("language-capabilities")))
                .map(|rect| rect.intersect(self.ctx.screen_rect()).center())
                .unwrap_or(self.ctx.screen_rect().center())
        } else if self.app.localization_ui.workbench.preview.is_some() {
            self.ctx.screen_rect().center()
        } else {
            self.viewport.center()
        }
    }

    pub(super) fn wheel(&mut self, point: Pos2, delta: f32) {
        self.frame(vec![
            Event::PointerMoved(point),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, delta),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        for _ in 0..80 {
            self.frame(vec![]);
            if self.scroll_delta == Vec2::ZERO {
                return;
            }
        }
        panic!(
            "native wheel smoothing did not settle: {:?}",
            self.scroll_delta
        );
    }

    pub(super) fn click(&mut self, label: &str) {
        let point = self.visible(label);
        self.click_point(point);
    }

    fn click_point(&mut self, point: Pos2) {
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        self.settle();
    }

    pub(super) fn replace(&mut self, current: &str, replacement: &str) {
        self.click(current);
        self.replace_focused(current, replacement);
    }

    fn replace_field(&mut self, id: egui::Id, current: &str, replacement: &str) {
        self.click_field(id, current);
        self.replace_focused(current, replacement);
    }

    pub(super) fn click_field(&mut self, id: egui::Id, current: &str) {
        let mut stable: Option<Rect> = None;
        let mut clicked = false;
        let mut last = Rect::NOTHING;
        for _ in 0..100 {
            self.frame(vec![]);
            let response = self
                .ctx
                .read_response(id)
                .expect("typed field exists in the selected detail");
            assert!(
                response.enabled(),
                "{current:?} is unexpectedly disabled: pending={}, selected={:?}",
                self.app.localization_ui.jobs.catalog_pending(),
                self.app.localization_ui.workbench.selected
            );
            let rect = response.rect;
            last = rect;
            let visible = rect
                .intersect(self.viewport)
                .intersect(self.ctx.screen_rect());
            if visible.is_positive()
                && visible.height() + 0.5 >= rect.height()
                && visible.width() + 0.5 >= rect.width()
            {
                if stable.is_some_and(|old| {
                    old.min.distance(rect.min) < 0.25 && old.max.distance(rect.max) < 0.25
                }) {
                    self.click_point(visible.center());
                    clicked = true;
                    break;
                }
                stable = Some(rect);
            } else {
                stable = None;
                self.wheel(
                    self.viewport.center(),
                    if rect.top() < self.viewport.top() {
                        (self.viewport.top() - rect.top()).min(38.0)
                    } else {
                        (self.viewport.bottom() - rect.bottom()).max(-38.0)
                    },
                );
            }
        }
        assert!(
            clicked,
            "real short viewport cannot reach typed field {current:?}: field={last:?}, viewport={:?}, remaining scroll={:?}", self.viewport, self.scroll_delta
        );
        assert_eq!(
            self.ctx.memory(|memory| memory.focused()),
            Some(id),
            "clicked the actual editable field, not its identical source label"
        );
    }

    fn replace_focused(&mut self, current: &str, replacement: &str) {
        let id = self
            .ctx
            .memory(|memory| memory.focused())
            .unwrap_or_else(|| panic!("visible text input {current:?} owns focus; selected={:?}, pending={}, detail={}, screen={:?}",
                self.app.localization_ui.workbench.selected, self.app.localization_ui.jobs.catalog_pending(),
                self.app.localization_ui.workbench.detail, self.ctx.screen_rect()));
        let mut editor = egui::TextEdit::load_state(&self.ctx, id).expect("clicked a TextEdit");
        editor
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(current.chars().count()),
            )));
        egui::TextEdit::store_state(&self.ctx, id, editor);
        self.frame(vec![Event::Text(replacement.into())]);
        self.settle();
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.app.project.root);
    }
}

fn painted(output: &egui::FullOutput, label: &str) -> Option<(Rect, Rect)> {
    fn text_rect(shape: &egui::Shape, label: &str) -> Option<Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.galley.rect.translate(text.pos.to_vec2()))
            }
            egui::Shape::Vec(shapes) => shapes
                .iter()
                .rev()
                .find_map(|shape| text_rect(shape, label)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .rev()
        .find_map(|shape| text_rect(&shape.shape, label).map(|rect| (rect, shape.clip_rect)))
}

#[test]
fn localization_short_native_200_percent_reaches_id_translation_preview_cancel_apply_and_save() {
    let mut h = Harness::new(
        "event start\n  待分配 ID 😀\n  已有源文 #wl-localization:existing\n  -> END\n",
    );
    h.visible("源语言");
    h.replace("zh-Hant", "fr");
    assert_eq!(h.app.localization_ui.target_locale, "fr");
    h.replace("fr", "zh-Hant");
    assert!(!h.app.localization_ui.has_unsubmitted_work());
    h.click("全部状态");
    h.click("缺 ID · 1");
    assert_eq!(
        h.app.localization_ui.workbench.status,
        Some(LocalizationStatus::MissingId)
    );
    assert_eq!(
        h.app.localization_ui.workbench.page.as_ref().unwrap().total,
        1
    );
    h.click("当前源文与译文");
    h.replace("例如 chapter01_welcome", "greeting");
    let baseline = h.app.project.content_baseline();
    h.click("预览此 ID 修改");
    h.visible("应用到工程（可撤销）");
    h.click("取消预览，保留输入");
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h
        .app
        .localization_ui
        .workbench
        .id_inputs
        .values()
        .any(|id| id == "greeting"));
    h.click("预览此 ID 修改");
    h.click("应用到工程（可撤销）");
    assert!(h
        .app
        .project
        .document(&h.app.project.entry)
        .unwrap()
        .contains("#wl-localization:greeting"));
    assert!(h
        .app
        .localization_ui
        .workbench
        .id_inputs
        .values()
        .all(String::is_empty));
    assert!(!h.app.localization_ui.workbench.has_input());
    assert!(!h.app.localization_ui.has_unsubmitted_work());
    h.click("字符串目录");
    h.click("缺 ID");
    h.click("全部状态");
    h.click("当前源文与译文");
    assert_eq!(
        h.app
            .localization_ui
            .workbench
            .selected_entry()
            .unwrap()
            .id
            .as_deref(),
        Some("greeting")
    );
    let key = editing::draft_key("zh-Hant", "greeting");
    let field = egui::Id::new(("localization-part-text", &h.app.project.root, &key, 0usize));
    h.replace_field(field, "待分配 ID 😀", "繁體譯文😀\nSecond line");
    assert_eq!(h.app.localization_ui.workbench.drafts.len(), 1);
    h.click("高级 JSON 交换");
    assert!(h.app.localization_ui.has_unsubmitted_work());
    h.click("译文目录");
    h.visible("繁體譯文😀\nSecond line");
    h.click("预览 1 项译文");
    let baseline = h.app.project.content_baseline();
    h.click("取消预览，保留输入");
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert_eq!(h.app.localization_ui.workbench.drafts.len(), 1);
    h.click("预览 1 项译文");
    h.click("应用到工程（可撤销）");
    assert!(h.app.localization_ui.workbench.drafts.is_empty());
    assert!(!h.app.localization_ui.has_unsubmitted_work());
    assert!(h.app.project.is_dirty());
    h.click("保存全部");
    assert!(!h.app.project.is_dirty());
    let sidecar = h.app.project.root.join(".world/localization/zh-Hant.json");
    assert!(std::fs::read_to_string(sidecar)
        .unwrap()
        .contains("繁體譯文😀"));
}

#[test]
fn localization_short_native_200_percent_reaches_advanced_inputs_and_import_confirmation() {
    let mut h = Harness::new("event start\n  Source #wl-localization:line0\n  -> END\n");
    let mut selection = h.app.localization_ui.selection();
    selection.string_ids = vec!["line0".into()];
    let mut exchange = h
        .app
        .project
        .preview_localization_export(&selection)
        .unwrap()
        .exchange;
    exchange.entries[0].translation_parts = Some(vec![LocalizationPart::Text {
        text: "交換譯文😀".into(),
    }]);
    let json = serde_json::to_string(&exchange).unwrap();
    h.click("高级 JSON 交换");
    h.replace("welcome\nreply", "line0");
    h.visible("选择 JSON 交换文件…");
    h.replace("选择 .json 文件，或粘贴 UTF-8 JSON", &json);
    h.click("预览导入");
    assert!(
        h.app
            .localization_ui
            .import_plan
            .as_ref()
            .unwrap()
            .can_apply
    );
    let baseline = h.app.project.content_baseline();
    h.click("复核通过 · 确认导入…");
    h.visible("确认并原子导入");
    h.click("取消导入");
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert_eq!(h.app.localization_ui.exchange_json, json);
    assert!(h.app.localization_ui.has_unsubmitted_work());
    h.click("复核通过 · 确认导入…");
    h.click("确认并原子导入");
    assert_ne!(h.app.project.content_baseline(), baseline);
    assert!(!h.app.localization_ui.has_unsubmitted_work());
    h.click("保存全部");
    assert!(!h.app.project.is_dirty());
    assert!(h
        .app
        .project
        .root
        .join(".world/localization/zh-Hant.json")
        .exists());
    h.click("预览导出");
    assert!(
        h.app
            .localization_ui
            .export_plan
            .as_ref()
            .unwrap()
            .can_export
    );
    h.visible("导出 UTF-8 JSON…");
    h.click("明确文件路径");
    h.replace(
        "新导出文件完整路径",
        "/tmp/pending-localization-export.json",
    );
    h.visible("导出到新路径");
    h.replace(
        "已有 UTF-8 JSON 完整路径",
        "/tmp/pending-localization-import.json",
    );
    h.visible("从此路径载入");
    assert_eq!(
        h.app.localization_ui.export_path,
        "/tmp/pending-localization-export.json"
    );
    assert_eq!(
        h.app.localization_ui.import_path,
        "/tmp/pending-localization-import.json"
    );
}

#[test]
fn localization_id_preview_expansion_keeps_before_after_source_scroll_reachable() {
    let mut source = "event start\n".to_owned();
    for index in 0..40 {
        source.push_str(&format!("  尚无身份的源文 {index} 😀\n"));
    }
    source.push_str("  -> END\n");
    let mut h = Harness::new(&source);
    h.physical = egui::vec2(1188.0, 848.0);
    h.app.personal.settings.appearance.ui_scale = 1.0;
    for _ in 0..4 {
        h.frame(vec![]);
    }
    let output = h.frame(vec![]);
    assert_eq!(output.pixels_per_point, 1.0);
    assert_eq!(h.ctx.screen_rect().size(), h.physical);
    h.viewport = painted(&output, "本地化工作台").unwrap().1;
    h.replace("例如 chapter01_welcome", "firstid");
    let baseline = h.app.project.content_baseline();
    h.click("预览此 ID 修改");
    h.click("查看修改前后源码");
    h.visible("修改前");
    h.visible("修改后");
    h.visible("应用到工程（可撤销）");
    h.click("取消预览，保留输入");
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h
        .app
        .localization_ui
        .workbench
        .id_inputs
        .values()
        .any(|id| id == "firstid"));
    assert!(h.app.localization_ui.has_unsubmitted_work());
}
