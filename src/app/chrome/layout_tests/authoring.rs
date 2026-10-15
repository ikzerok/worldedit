//! 真实 App 的新动作条矩阵；RawInput 点数显式换算，不作为物理 GUI/IME 证据。
use super::*;
use crate::chrome::action_layout::{trace, ActionRect};
use crate::theme::{Density, PaletteModeSupport};
use worldline_core::project::Project;

mod paint;
mod states;
const STYLES: [StylePreset; 5] = [
    StylePreset::Studio,
    StylePreset::Manuscript,
    StylePreset::Technical,
    StylePreset::Focus,
    StylePreset::Ledger,
];
const LONG_TITLE: &str = "长中文书名：雾港／Latin/path／全角，组合 e\u{301} 与同名章节的书稿";

struct Harness {
    ctx: egui::Context,
    app: WorldeditApp,
    size: egui::Vec2,
}
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.app.project.root);
    }
}
impl Harness {
    fn new(drafts: bool, dirty: bool) -> Self {
        let (ctx, mut app) = app();
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "chrome-authoring-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ));
        std::fs::create_dir(&root).unwrap();
        app.project = Project::new(&root);
        app.active_file = app.project.entry.clone();
        app.project
            .set_text(
                &app.active_file,
                concat!(
                    "character traveler as \"旅人\"\n",
                    "event arrival as \"抵达\"\n  第一段正文。\n  -> END\n",
                    "event departure as \"离港\"\n  第二段正文。\n  -> END\n",
                    "event home as \"归来\"\n  第三段正文。\n  -> END\n",
                )
                .into(),
            )
            .unwrap();
        app.project.create_authoring_document(&root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.10","required_features":["presentation.manuscripts.v1"],"maps":{},"graph_views":{},"manuscripts":{"novel":".world/manuscripts/novel.json"}}"#.to_vec()).unwrap();
        let manuscript = serde_json::json!({
            "schema_version":1, "id":"novel", "title":LONG_TITLE,
            "entries":[
                {"id":"first", "kind":"chapter", "title":"同名章节", "target_ref":{"kind":"event","id":"arrival"}},
                {"id":"second", "kind":"chapter", "title":"同名章节", "target_ref":{"kind":"event","id":"departure"}},
                {"id":"third", "kind":"chapter", "title":"归来", "target_ref":{"kind":"event","id":"home"}}
            ]
        });
        app.project
            .create_authoring_document(
                &root.join(".world/manuscripts/novel.json"),
                serde_json::to_vec(&manuscript).unwrap(),
            )
            .unwrap();
        app.project.save().unwrap();
        app.saved_location = true;
        if dirty {
            let source = format!(
                "{}// 已应用待保存\n",
                app.project.document(&app.active_file).unwrap()
            );
            app.project.set_text(&app.active_file, source).unwrap();
        }
        app.reset_views();
        app.recompile();
        app.tab = Tab::Manuscript;
        app.personal.settings.appearance.reduce_motion = true;
        app.restore_manuscript_session(serde_json::from_value(serde_json::json!({
            "manuscript_id":"novel", "selected_id":"first", "mode":"source", "preview_open":false
        })).unwrap());
        if drafts {
            let mut buffer = app
                .project
                .open_source_writing_buffer(&app.active_file)
                .unwrap();
            buffer.replace_source(format!("{}// 未应用正文草稿\n", buffer.source()));
            app.manuscript.restore_writing_buffers(&[buffer]);
        }
        Self {
            ctx,
            app,
            size: egui::vec2(1280.0, 760.0),
        }
    }
    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        render(&self.ctx, &mut self.app, self.size.x, self.size.y, events)
    }
    fn full_frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        let modifiers = events
            .iter()
            .find_map(|event| match event {
                Event::Key { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or(egui::Modifiers::NONE);
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            modifiers,
            focused: true,
            events,
            ..Default::default()
        };
        eframe::App::raw_input_hook(&mut self.app, &self.ctx, &mut raw);
        self.ctx.run(raw, |ctx| {
            eframe::App::update(&mut self.app, ctx, &mut eframe::Frame::_new_kittest());
        })
    }
    fn settle(&mut self) -> egui::FullOutput {
        for _ in 0..3 {
            self.frame(vec![]);
        }
        self.frame(vec![])
    }
    fn response(&self, role: &str) -> ActionRect {
        trace(&self.ctx)
            .into_iter()
            .find(|r| r.role == role)
            .unwrap_or_else(|| panic!("missing {role}"))
    }
    fn click_role(&mut self, role: &str) {
        let pos = self.response(role).rect.center();
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        self.settle();
    }
    fn unchanged(&self) -> (String, Vec<(std::path::PathBuf, String)>) {
        let mut drafts: Vec<_> = self
            .app
            .manuscript
            .writing_buffers()
            .into_iter()
            .filter(|buffer| buffer.is_changed())
            .map(|buffer| (buffer.path().to_owned(), buffer.source().to_owned()))
            .collect();
        drafts.sort();
        (self.app.project.content_baseline(), drafts)
    }
    fn assert_geometry(&self) {
        let viewport = Rect::from_min_size(Pos2::ZERO, self.size);
        let responses = trace(&self.ctx);
        assert!(!responses.is_empty());
        for response in &responses {
            assert!(
                response.rect.is_finite() && response.rect.is_positive(),
                "{response:?}"
            );
            assert!(
                viewport.expand(1.0).contains_rect(response.rect),
                "viewport {viewport:?}: {response:?}"
            );
            assert!(
                response.clip.expand(1.0).contains_rect(response.rect),
                "clipped {response:?}"
            );
            if response.role != "manuscript/state" {
                assert!(
                    response.rect.width() >= 20.0 && response.rect.height() >= 20.0,
                    "small target {response:?}"
                );
            }
        }
        for (index, first) in responses.iter().enumerate() {
            for second in &responses[index + 1..] {
                assert!(
                    !first.rect.shrink(0.5).intersects(second.rect.shrink(0.5)),
                    "overlap {first:?}/{second:?}"
                );
            }
        }
        for role in ["global/save", "global/save-state", "global/commands"] {
            assert!(responses
                .iter()
                .any(|response| response.role == role && response.enabled));
        }
        let source = egui::Id::new(("writing-source", &self.app.active_file, "event", "arrival"));
        let response = self
            .ctx
            .read_response(source)
            .expect("opened chapter retains its real TextEdit");
        assert!(
            response.interact_rect.intersect(viewport).height() >= 20.0,
            "a usable body viewport remains: {:?}; viewport={viewport:?}; appearance={:?}",
            response.interact_rect,
            self.app.personal.appearance()
        );
    }
}

#[test]
fn action_bars_all_eighty_five_palette_style_pairs_keep_both_edit_states() {
    let mut count = 0;
    for palette in PaletteId::ALL {
        let modes: &[ThemeMode] = match palette.mode_support() {
            PaletteModeSupport::Both => &[ThemeMode::Light, ThemeMode::Dark],
            PaletteModeSupport::LightOnly => &[ThemeMode::Light],
            PaletteModeSupport::DarkOnly => &[ThemeMode::Dark],
        };
        for mode in modes {
            for style in STYLES {
                let mut h = Harness::new(true, true);
                h.app.personal.settings.appearance = AppearancePreferences {
                    palette,
                    theme: *mode,
                    style,
                    reduce_motion: true,
                    ..Default::default()
                };
                let before = h.unchanged();
                let output = h.settle();
                h.assert_geometry();
                assert!(text_position(&output, "输入待应用 · 工程待保存").is_some());
                let state = h.response("global/save-state");
                paint::assert_state_contrast(&output, "输入待应用 · 工程待保存", state.rect);
                let hover = h.frame(vec![Event::PointerMoved(state.rect.center())]);
                paint::assert_state_contrast(&hover, "输入待应用 · 工程待保存", state.rect);
                h.ctx.memory_mut(|memory| memory.request_focus(state.id));
                let output = h.frame(vec![Event::PointerGone]);
                assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(state.id));
                assert_focus_paint(&output, state.rect, theme::resolved(&h.ctx).colors.focus);
                assert_eq!(h.unchanged(), before);
                let resolved = theme::resolved(&h.ctx);
                assert_eq!(resolved.preferences.style, style);
                assert_eq!(resolved.effective_mode, *mode);
                count += 1;
            }
        }
    }
    assert_eq!(count, 85);
}

#[test]
fn action_bars_one_hundred_eighty_client_zoom_density_geometries_preserve_content() {
    let mut count = 0;
    let mut failures = Vec::new();
    for style in STYLES {
        for client in [
            egui::vec2(760.0, 540.0),
            egui::vec2(1280.0, 760.0),
            egui::vec2(1920.0, 1080.0),
        ] {
            for zoom in [0.8, 1.0, 1.5, 2.0] {
                for density in [Density::Compact, Density::Standard, Density::Spacious] {
                    // 每一格各有独立 Context / 工程；某格失败仍继续保留其他格证据。
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let mut h = Harness::new(true, true);
                        h.app.personal.settings.appearance = AppearancePreferences {
                            style,
                            density,
                            ui_scale: zoom,
                            body_size: 36.0,
                            source_size: 28.0,
                            reduce_motion: true,
                            ..Default::default()
                        };
                        // 这里的客户端逻辑尺寸只除一次应用 zoom；没有宿主 DPI 的猜测或重复缩放。
                        h.size = client / zoom;
                        h.ctx.set_zoom_factor(zoom);
                        let before = h.unchanged();
                        let settings = serde_json::to_value(&h.app.personal.settings).unwrap();
                        h.settle();
                        h.assert_geometry();
                        assert_eq!(
                            h.unchanged(),
                            before,
                            "{style:?}/{client:?}/{zoom}/{density:?}"
                        );
                        assert_eq!(
                            serde_json::to_value(&h.app.personal.settings).unwrap(),
                            settings
                        );
                    }));
                    if let Err(error) = result {
                        let message = error
                            .downcast_ref::<String>()
                            .map(String::as_str)
                            .or_else(|| error.downcast_ref::<&str>().copied())
                            .unwrap_or("non-string panic");
                        failures.push(format!(
                            "{style:?}/{client:?}/{zoom}/{density:?}: {message}"
                        ));
                    }
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, 180);
    assert!(
        failures.is_empty(),
        "180-case geometry matrix failures:\n{}",
        failures.join("\n")
    );
}

fn assert_focus_paint(output: &egui::FullOutput, target: Rect, color: Color32) {
    fn find(shape: &egui::Shape, clip: Rect, target: Rect, color: Color32) -> bool {
        match shape {
            egui::Shape::Rect(rect) => {
                rect.stroke.color == color
                    && rect.stroke.width >= 2.0
                    && target.expand(1.0).contains_rect(rect.rect)
                    && clip.contains_rect(rect.rect)
            }
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| find(shape, clip, target, color)),
            _ => false,
        }
    }
    assert!(
        output
            .shapes
            .iter()
            .any(|s| find(&s.shape, s.clip_rect, target, color)),
        "focus outline must be fully painted: {target:?}"
    );
}
