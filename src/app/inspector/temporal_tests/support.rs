use crate::app::{Tab, WorldeditApp};
use egui::{pos2, vec2, Event, FullOutput, PointerButton, RawInput, Rect};
use worldline_core::project::Project;

pub(super) struct Harness {
    pub ctx: egui::Context,
    pub app: WorldeditApp,
    pub size: egui::Vec2,
}
impl Harness {
    pub fn new(version: &str) -> Self {
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.0);
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = WorldeditApp::new(&creation, None);
        let root = std::env::temp_dir().join(format!(
            "worldedit-predecessor-form-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        app.project = Project::new(&root);
        let entry = app.project.entry.clone();
        app.project.documents.retain(|path, _| path == &entry);
        let source = concat!(
            "period year as \"年\"\n",
            "period summer as \"夏\" within year\n",
            "period autumn as \"秋\" within year\n",
            "period deep as \"深秋\" within autumn\n",
            "period other as \"另根\"\n",
            "event arrival during summer at 80 as \"同名来信\"\n  -> archive\n",
            "event archive during autumn follows arrival at 20 as \"档案馆密谈\"\n  -> END\n",
            "event successor during autumn follows archive at 90 as \"后继\"\n  -> END\n",
            "event foreign during other as \"独立根事件\"\n  -> END\n",
        );
        let source = if version == "1.13" {
            source.to_owned()
        } else {
            source.replace("event arrival during summer", "event arrival during autumn")
        };
        app.project.set_text(&entry, source).unwrap();
        let peer = app
            .project
            .add_file(std::path::Path::new("chapters/peer.wl"))
            .unwrap();
        app.project
            .set_text(
                &peer,
                "event peer during deep as \"同名来信\"\n  -> END\n".into(),
            )
            .unwrap();
        app.project.create_authoring_document(&root.join(".world/project.json"), serde_json::to_vec(&serde_json::json!({
            "schema_version":1,"language_version":version,"required_features":["presentation.manuscripts.v1"],
            "manuscripts":{"book":".world/manuscripts/book.json"}
        })).unwrap()).unwrap();
        app.project.create_authoring_document(&root.join(".world/manuscripts/book.json"), br#"{"schema_version":1,"id":"book","title":"Book","entries":[{"id":"later","kind":"chapter","title":"Later","target_ref":{"kind":"event","id":"archive"}},{"id":"earlier","kind":"chapter","title":"Earlier","target_ref":{"kind":"event","id":"arrival"}}]}"#.to_vec()).unwrap();
        app.active_file = entry;
        app.tab = Tab::Timeline;
        app.reset_views();
        app.recompile();
        assert!(
            !app.snapshot.as_ref().unwrap().result.has_errors(),
            "{:?}",
            app.snapshot.as_ref().unwrap().result.diagnostics
        );
        app.project.save().unwrap();
        app.saved_location = true;
        app.select_event("archive");
        Self {
            ctx,
            app,
            size: vec2(1188.0, 848.0),
        }
    }

    pub fn frame(&mut self, events: Vec<Event>) -> FullOutput {
        self.ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), self.size)),
                events,
                ..Default::default()
            },
            |ctx| {
                self.app.status_bar(ctx);
                self.app.event_inspector(ctx);
                egui::CentralPanel::default().show(ctx, |_| {});
            },
        )
    }
    pub fn output(&mut self) -> FullOutput {
        for _ in 0..3 {
            self.frame(Vec::new());
        }
        self.frame(Vec::new())
    }
    pub fn text(&mut self) -> String {
        text(&self.output())
    }
    pub fn click(&mut self, label: &str) {
        let output = self.output();
        let point = position(&output, label)
            .unwrap_or_else(|| panic!("未显示可点击文字 {label}: {}", text(&output)));
        self.click_at(point);
    }
    pub fn click_at(&mut self, point: egui::Pos2) {
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
    }
    pub fn period(&mut self, id: &str, choice: &str) {
        self.click(id);
        self.click(choice);
    }
    pub fn replace(&mut self, label: &str, replacement: &str) {
        self.click(label);
        self.frame(vec![
            key(egui::Key::A, true, egui::Modifiers::COMMAND),
            key(egui::Key::A, false, egui::Modifiers::COMMAND),
            Event::Text(replacement.into()),
        ]);
    }
    pub fn filter(&mut self, query: &str) {
        let current = self
            .app
            .event_editor
            .as_ref()
            .unwrap()
            .predecessor_query
            .clone();
        self.click(if current.is_empty() {
            "筛选候选：名称 / ID / 时段 / 根 / 来源"
        } else {
            &current
        });
        self.frame(vec![
            key(egui::Key::A, true, egui::Modifiers::COMMAND),
            key(egui::Key::A, false, egui::Modifiers::COMMAND),
            Event::Text(query.into()),
        ]);
    }
    pub fn wheel(&mut self, point: egui::Pos2, amount: f32) {
        self.frame(vec![
            Event::PointerMoved(point),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, amount),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        for _ in 0..8 {
            self.frame(Vec::new());
        }
    }
    pub fn predecessors(&self) -> Vec<String> {
        self.app
            .event_editor
            .as_ref()
            .unwrap()
            .draft
            .predecessors
            .clone()
    }
    pub fn edges(&self) -> Vec<(String, String)> {
        let mut edges: Vec<_> = self
            .app
            .snapshot
            .as_ref()
            .unwrap()
            .result
            .analysis
            .timeline
            .edges
            .iter()
            .map(|edge| (edge.before.clone(), edge.after.clone()))
            .collect();
        edges.sort();
        edges
    }
    pub fn fingerprint(&self) -> u64 {
        self.app
            .snapshot
            .as_ref()
            .unwrap()
            .result
            .analysis
            .fingerprint
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.app.project.root);
    }
}

pub(super) fn key(key: egui::Key, pressed: bool, modifiers: egui::Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers,
    }
}
pub(super) fn text(output: &FullOutput) -> String {
    fn append(shape: &egui::Shape, result: &mut String) {
        match shape {
            egui::Shape::Text(text) => {
                result.push_str(&text.galley.job.text);
                result.push('\n');
            }
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| append(shape, result)),
            _ => {}
        }
    }
    let mut result = String::new();
    output
        .shapes
        .iter()
        .for_each(|shape| append(&shape.shape, &mut result));
    result
}
pub(super) fn position(output: &FullOutput, needle: &str) -> Option<egui::Pos2> {
    fn find(shape: &egui::Shape, needle: &str, clip: Rect) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.job.text == needle => {
                let rect = text
                    .galley
                    .rect
                    .translate(text.pos.to_vec2())
                    .intersect(clip);
                rect.is_positive().then(|| rect.center())
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, needle, clip)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|shape| find(&shape.shape, needle, shape.clip_rect))
}
