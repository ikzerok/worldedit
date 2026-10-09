use super::*;
use std::path::Path;

pub(super) const CASES: [(&str, &str, u32, &str); 6] = [
    ("root_body", "world.wl", 4, "text"),
    ("root_spoken", "world.wl", 5, "say"),
    ("root_go", "world.wl", 7, "choice"),
    ("body", "chapters/body.wl", 11, "text"),
    ("spoken", "chapters/body.wl", 12, "say"),
    ("go", "chapters/body.wl", 13, "choice"),
];
const ROOT: &str = concat!(
    "let n = 0\ncharacter speaker as \"林舟\"\nevent start\n",
    "  Root🌙 {rnd(1, 1000)} #wl-localization:root_body\n",
    "  say speaker \"Root spoken {n}\" #wl-localization:root_spoken\n",
    "include \"chapters/body.wl\"\n",
    "  choice \"Root go {rnd(1, 1000)}\" #wl-localization:root_go\n",
    "    set n = 1\n    -> END\n",
);
const INCLUDED: &str = concat!(
    "\n\n\n\n\n\n\n\n\n\n",
    "  Included🌦️ {rnd(1, 1000)} #wl-localization:body\n",
    "  say speaker \"Included spoken {n}\" #wl-localization:spoken\n",
    "  choice \"Included go {rnd(1, 1000)}\" #wl-localization:go\n",
    "    set n = 2\n    -> END\n",
);

pub(super) struct Harness {
    pub ctx: egui::Context,
    pub app: WorldeditApp,
    time: f64,
    viewport: Rect,
    scroll_delta: egui::Vec2,
}
impl Harness {
    pub fn new(crlf: bool) -> Self {
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = WorldeditApp::new(&creation, None);
        let (mut project, mut state) = workbench_tests::fixture(1);
        let manifest = project.root.join(".world/project.json");
        let mut config: serde_json::Value =
            serde_json::from_slice(project.authoring_document(&manifest).unwrap().bytes()).unwrap();
        config["language_version"] = "1.11".into();
        project
            .set_authoring_document(&manifest, serde_json::to_vec(&config).unwrap())
            .unwrap();
        let child = project.add_file(Path::new("chapters/body.wl")).unwrap();
        for (path, source) in [(project.entry.clone(), ROOT), (child, INCLUDED)] {
            project
                .set_text(
                    &path,
                    if crlf {
                        source.replace('\n', "\r\n")
                    } else {
                        source.into()
                    },
                )
                .unwrap();
        }
        project.save().unwrap();
        state.string_ids = CASES
            .iter()
            .map(|(id, ..)| *id)
            .collect::<Vec<_>>()
            .join("\n");
        app.project = project;
        app.active_file = app.project.entry.clone();
        app.reset_views();
        app.recompile();
        app.saved_location = true;
        app.personal.pending_restore = false;
        app.localization_ui = state;
        app.tab = Tab::Localization;
        app.personal.settings.appearance.ui_scale = 1.0;
        app.personal.settings.appearance.reduce_motion = true;
        assert!(
            !app.snapshot.as_ref().unwrap().result.has_errors(),
            "{:?}",
            app.snapshot.as_ref().unwrap().result.diagnostics
        );
        assert_eq!(
            app.snapshot.as_ref().unwrap().result.program.events[0]
                .body
                .iter()
                .filter(|stmt| matches!(stmt, worldline_core::ast::Stmt::Say(_)))
                .count(),
            2
        );
        let mut h = Self {
            ctx,
            app,
            time: 0.0,
            viewport: Rect::NOTHING,
            scroll_delta: egui::Vec2::ZERO,
        };
        for _ in 0..4 {
            h.frame(vec![]);
        }
        h.settle();
        let output = h.frame(vec![]);
        h.viewport = painted(&output, "本地化工作台").last().unwrap().1;
        assert!(
            h.viewport.top() > 40.0 && h.viewport.height() < 950.0,
            "full global chrome must precede workbench"
        );
        h
    }
    pub fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.time += 1.0 / 60.0;
        let screen = Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(750.0, 950.0) / self.ctx.pixels_per_point(),
        );
        let mut raw = egui::RawInput {
            screen_rect: Some(screen),
            time: Some(self.time),
            modifiers: if events
                .iter()
                .any(|event| matches!(event, Event::Key { modifiers, .. } if modifiers.alt))
            {
                egui::Modifiers::ALT
            } else {
                egui::Modifiers::NONE
            },
            events,
            ..Default::default()
        };
        let viewport = raw.viewports.entry(egui::ViewportId::ROOT).or_default();
        viewport.native_pixels_per_point = Some(1.0);
        viewport.inner_rect = Some(screen);
        viewport.outer_rect = Some(screen);
        viewport.focused = Some(true);
        eframe::App::raw_input_hook(&mut self.app, &self.ctx, &mut raw);
        let mut scroll_delta = egui::Vec2::ZERO;
        let output = self.ctx.run(raw, |ctx| {
            // Observe before ScrollArea consumes this frame's delta.
            scroll_delta = ctx.input(|input| input.smooth_scroll_delta);
            eframe::App::update(&mut self.app, ctx, &mut eframe::Frame::_new_kittest())
        });
        self.scroll_delta = scroll_delta;
        output
    }
    pub fn settle(&mut self) {
        self.frame(vec![]);
        jobs::settle(
            &self.app.project,
            &mut self.app.localization_ui,
            self.app.version,
        );
        self.frame(vec![]);
    }
    fn wheel(&mut self, point: Pos2, delta: f32) {
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
            if self.scroll_delta == egui::Vec2::ZERO {
                return;
            }
        }
        panic!("native wheel smoothing did not settle");
    }
    fn point(&mut self, label: &str, last: bool) -> Pos2 {
        let point = if self.app.localization_ui.workbench.preview.is_some() {
            self.ctx.screen_rect().center()
        } else {
            self.viewport.center()
        };
        // Start at the end when the import row intentionally repeats an export's source label.
        self.wheel(point, if last { -100_000.0 } else { 100_000.0 });
        let mut seen = String::new();
        for _ in 0..160 {
            let output = self.frame(vec![]);
            let mut candidates = painted(&output, label);
            if last {
                candidates.reverse();
            }
            for (rect, clip) in candidates {
                let visible = rect.intersect(clip).intersect(self.ctx.screen_rect());
                if visible.is_positive()
                    && visible.height() + 0.5 >= rect.height().min(clip.height())
                    && visible.width() + 0.5 >= rect.width().min(clip.width())
                {
                    return visible.center();
                }
            }
            seen = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.text()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" | ");
            self.wheel(point, if last { 100.0 } else { -100.0 });
        }
        panic!("full App cannot reach {label}: {seen}");
    }
    pub fn click(&mut self, label: &str) {
        self.click_order(label, false);
    }
    pub fn click_last(&mut self, label: &str) {
        self.click_order(label, true);
    }
    fn click_order(&mut self, label: &str, last: bool) {
        let point = self.point(label, last);
        let hovered = self.frame(vec![Event::PointerMoved(point)]);
        if label.starts_with("源文 ")
            || label.starts_with("定位来源 ")
            || label.starts_with("定位诊断 ")
        {
            assert_eq!(
                hovered.platform_output.cursor_icon,
                egui::CursorIcon::PointingHand,
                "visible source glyphs must hover the real enabled Link: {label} at {point:?}"
            );
        }
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
    pub fn back(&mut self) {
        for pressed in [true, false] {
            self.frame(vec![Event::Key {
                key: egui::Key::ArrowLeft,
                physical_key: Some(egui::Key::ArrowLeft),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::ALT,
            }]);
        }
        assert_eq!(self.app.tab, Tab::Localization);
        self.settle();
    }
    pub fn catalog(&mut self, id: &str) {
        self.app
            .localization_ui
            .open_translation("zh-Hant", Some(id));
        self.app.tab = Tab::Localization;
        self.settle();
    }
    pub fn exchange(&self) -> LocalizationExchange {
        let mut exchange = self
            .app
            .project
            .preview_localization_export(&self.app.localization_ui.selection())
            .unwrap()
            .exchange;
        for entry in &mut exchange.entries {
            entry.translation_parts = Some(entry.source_parts.clone());
        }
        exchange
    }
    pub fn import(&mut self, exchange: LocalizationExchange) {
        if !self.app.localization_ui.advanced {
            self.click("高级 JSON 交换");
        }
        self.app.localization_ui.exchange_json = serde_json::to_string(&exchange).unwrap();
        self.app.localization_ui.invalidate_import();
        self.click("预览导入");
        assert!(
            self.app.localization_ui.import_plan.is_some(),
            "{:?}",
            self.app.localization_ui.status
        );
    }
    pub fn assert_source(&mut self, source: &LocalizationSource, id: &str) {
        assert_eq!(self.app.tab, Tab::Edit, "{:?}", self.app.message);
        let path = self.app.project.root.join(&source.file);
        assert_eq!(self.app.active_file, path);
        assert!(self.app.jump.is_none());
        for _ in 0..3 {
            self.frame(vec![]);
        }
        let text = self.app.project.document(&path).unwrap();
        let expected = text.lines().nth(source.line as usize - 1).unwrap().trim();
        assert!(expected.contains(&format!("#wl-localization:{id}")));
        let range = egui::TextEdit::load_state(&self.ctx, egui::Id::new(("source", &path)))
            .unwrap()
            .cursor
            .char_range()
            .unwrap();
        let low = range.primary.index.min(range.secondary.index);
        let high = range.primary.index.max(range.secondary.index);
        assert_eq!(
            text.chars().skip(low).take(high - low).collect::<String>(),
            expected
        );
        assert!(!expected.contains("set n ="));
    }
    pub fn unchanged(&self) -> serde_json::Value {
        let state = &self.app.localization_ui;
        let disk = [
            self.app.project.entry.clone(),
            self.app.project.root.join("chapters/body.wl"),
            self.app.project.root.join(".world/project.json"),
        ]
        .iter()
        .map(|path| std::fs::read(path).unwrap())
        .collect::<Vec<_>>();
        serde_json::json!({
            "baseline": self.app.project.content_baseline(), "version": self.app.version,
            "dirty": self.app.project.is_dirty(), "undo": self.app.history.len(), "redo": self.app.redo.len(),
            "json": state.exchange_json, "export": state.export_plan, "import": state.import_plan,
            "exchange": state.import_exchange,
            "drafts": state.workbench.drafts.iter().map(|(k,v)| (k,&v.edit)).collect::<Vec<_>>(),
            "disk": disk,
        })
    }
    pub fn request(&self) -> Request {
        let page = self.app.localization_ui.workbench.page.as_ref().unwrap();
        Request::catalog_entry(
            page,
            self.app.localization_ui.workbench.selected_entry().unwrap(),
        )
        .unwrap()
    }
    pub fn reject(&mut self, request: Request, message: &str) {
        let before = self.unchanged();
        let location = self.app.active_file.clone();
        let history = self.app.personal.history.len();
        self.app.open_localization_source(&self.ctx, request);
        assert_eq!(self.app.tab, Tab::Localization);
        assert_eq!(self.app.active_file, location);
        assert_eq!(self.app.personal.history.len(), history);
        assert_eq!(self.unchanged(), before);
        assert!(
            self.app.message.as_deref().unwrap().contains(message),
            "{:?}",
            self.app.message
        );
        assert!(self.app.localization_ui.status.as_ref().unwrap().is_err());
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.app.project.root);
    }
}
fn painted(output: &egui::FullOutput, label: &str) -> Vec<(Rect, Rect)> {
    fn text(shape: &egui::Shape, label: &str, clip: Rect, result: &mut Vec<(Rect, Rect)>) {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == label => {
                // egui's wrapped Label/Link hit rectangles exclude preceding widgets' indentation.
                // galley.rect includes that blank space; its center can hit the previous ID/status.
                for (index, row) in t.galley.rows.iter().enumerate() {
                    let rect = if index == 0 {
                        row.rect_without_leading_space()
                    } else {
                        row.rect()
                    };
                    result.push((rect.translate(t.pos.to_vec2()), clip));
                }
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    text(shape, label, clip, result);
                }
            }
            _ => {}
        }
    }
    let mut result = Vec::new();
    for shape in &output.shapes {
        text(&shape.shape, label, shape.clip_rect, &mut result);
    }
    result
}
