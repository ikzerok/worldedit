//! App-level production workflows with real disk fixtures and asynchronous jobs.
//! Filter/draft setup uses app state; generation, paging and delivery controls use
//! synthetic egui pointer events. These are not native-window or OS clipboard tests.
mod delivery;
mod guards;
mod queries;
use super::*;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};
use worldline_core::{localization::LocalizationPart, project::Project, TargetRef};

const CONFIRM: &str = "已核对范围、语言状态和上述交付字节，确认私密材料内容";
const PRIVATE: &str = "PRIVATE_DIRECTION_SENTINEL";
static SERIAL: Mutex<()> = Mutex::new(());
fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|error| error.into_inner())
}
struct Flow {
    ctx: egui::Context,
    app: WorldeditApp,
    directory: PathBuf,
    // Hold only the polling boundary so cancellation can be tested deterministically
    // against a real spawned job, including one whose result is already queued.
    hold_poll: bool,
}
impl Flow {
    fn new(lines: usize) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "production-flow-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root = directory.join("workspace");
        fs::create_dir_all(root.join(".world")).unwrap();
        let mut source = String::from(
            "let n = 1\ncharacter a as \"同名\"\ncharacter b as \"同名\"\ninclude \"shared.wl\"\nfragment left()\n  call leaf()\n  return\nfragment right()\n  call leaf()\n  return\nevent start with b\n  同名: unselected narration\n",
        );
        for index in 0..lines {
            source.push_str(&format!(
                "  say a \"Line {index:02}\" direction \"{PRIVATE}\" #wl-localization:main_{index}\n"
            ));
        }
        source.push_str("  say b \"OTHER_ROLE_SECRET\"\n  if n > 0\n    call left()\n    call right()\n  call left()\n  -> END\nevent other with a\n  say b \"UNSELECTED_SECRET\"\n  -> END\n");
        fs::write(root.join("world.wl"), source).unwrap();
        fs::write(root.join("shared.wl"), format!(
            "fragment leaf()\n  say a \"Shared {{n}}\" direction \"{PRIVATE}\" #wl-localization:leaf\n  return\n"
        )).unwrap();
        let manifest = json!({"schema_version":1,"language_version":"1.12",
            "required_features":["presentation.manuscripts.v1","content.localization.v1"],
            "manuscripts":{"book":".world/book.json"},"localizations":{"en":".world/en.json"}});
        let book = json!({"schema_version":1,"id":"book","title":"Book","entries":[
            {"id":"one","kind":"chapter","title":"Selected One","pov":{"kind":"character","id":"b"},"target_ref":{"kind":"event","id":"start"}},
            {"id":"two","kind":"chapter","title":"Selected Two","target_ref":{"kind":"event","id":"start"}},
            {"id":"other","kind":"chapter","title":"Excluded","target_ref":{"kind":"event","id":"other"}}
        ]});
        let mut locale = json!({"schema_version":1,"required_features":["content.localization.v1"],
            "source_locale":"zh","target_locale":"en","entries":{}});
        for (file, value) in [("project", &manifest), ("book", &book), ("en", &locale)] {
            fs::write(
                root.join(format!(".world/{file}.json")),
                serde_json::to_vec(value).unwrap(),
            )
            .unwrap();
        }
        let mut project = Project::open(&root).unwrap();
        let mut request = ProductionScriptRequest::new(ProductionScope::CurrentTarget {
            target: TargetRef::new("event", "start"),
        });
        request.speaker = Some(TargetRef::new("character", "a"));
        // Translation seeding is fixture construction, not the workflow under test.
        let seed = project
            .production_script_snapshot(&[], &[], &request)
            .unwrap();
        for row in seed.page(0, 100).unwrap().rows {
            locale["entries"][row.stable_line_id.unwrap()] = json!({
                "source_revision":row.source_revision,"translation_parts":row.source_parts
            });
        }
        if lines > 1 {
            locale["entries"]["main_1"]["translation_parts"] = json!([]);
        }
        project
            .set_authoring_document(
                &root.join(".world/en.json"),
                serde_json::to_vec(&locale).unwrap(),
            )
            .unwrap();
        project.save().unwrap();
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.0);
        let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        app.project = project;
        app.active_file = app.project.entry.clone();
        app.saved_location = true;
        app.recompile();
        let navigation = app
            .manuscript
            .plan_review_match(
                &TargetRef::new("event", "start"),
                &app.project.entry,
                &app.project,
            )
            .unwrap();
        app.manuscript
            .apply_writing_match(navigation, &app.project)
            .unwrap();
        app.tab = crate::app::Tab::Manuscript;
        app.manuscript.production.speaker = Some(TargetRef::new("character", "a"));
        app.open_production_script();
        assert!(app.project.entry.is_file());
        assert!(!app.project.is_dirty());
        Self {
            ctx,
            app,
            directory,
            hold_poll: false,
        }
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        let hold = self.hold_poll;
        self.ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 16000.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                if hold {
                    self.app.manuscript.production.polled_frame = Some(ctx.cumulative_frame_nr());
                }
                self.app.manuscript_tab(ctx);
            },
        )
    }
    fn click(&mut self, label: &str) -> egui::FullOutput {
        for _ in 0..3 {
            self.frame(vec![]);
        }
        let output = self.frame(vec![]);
        let position = output
            .shapes
            .iter()
            .find_map(|shape| position(&shape.shape, label))
            .unwrap_or_else(|| panic!("missing rendered control: {label}"));
        let mut output = output;
        for pressed in [true, false] {
            output = self.frame(vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        output
    }
    fn start(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            self.app.begin_production_script(&self.ctx);
            if self.app.manuscript.production.job.is_some() {
                return;
            }
            assert!(
                self.notice().contains("前一个有界作业"),
                "{}",
                self.notice()
            );
            assert!(
                Instant::now() < deadline,
                "previous production worker did not release BUSY"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn finish_job(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while self.app.manuscript.production.job.is_some() {
            assert!(
                Instant::now() < deadline,
                "production job timed out: {}",
                self.notice()
            );
            let _ = self.ctx.run(Default::default(), |ctx| {
                self.app.poll_production_script(ctx)
            });
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn generate(&mut self) {
        self.start();
        self.finish_job();
        assert!(self.app.production_is_current(), "{}", self.notice());
    }
    fn snapshot(&self) -> Arc<ProductionScriptSnapshot> {
        self.app
            .manuscript
            .production
            .snapshot
            .as_ref()
            .unwrap()
            .clone()
    }
    fn rows(&self) -> Vec<ProductionRow> {
        self.snapshot().page(0, 100).unwrap().rows
    }
    fn notice(&self) -> &str {
        self.app
            .manuscript
            .production
            .notice
            .as_deref()
            .unwrap_or("")
    }
    fn artifact(&self) -> Vec<u8> {
        self.app
            .manuscript
            .production
            .artifact
            .as_ref()
            .unwrap()
            .bytes()
            .to_vec()
    }
    fn preview(&mut self) {
        if !self.app.manuscript.production.export_open {
            self.click("准备私密交付");
        }
        self.click("预览完整交付字节");
    }
    fn confirm(&mut self) {
        assert!(!self.app.manuscript.production.confirmed);
        self.click(CONFIRM);
        assert!(self.app.manuscript.production.confirmed);
        self.app.checked_production_artifact(&self.ctx).unwrap();
    }
    fn stage(&mut self, from: &str, to: &str) {
        let entry = self.app.project.entry.clone();
        let buffer = self.app.manuscript.writing_buffers.get_mut(&entry).unwrap();
        assert!(buffer.source().contains(from));
        buffer.replace_source(buffer.source().replace(from, to));
        self.app.manuscript.invalidate_query_cache();
    }
    fn locale(&self) -> Value {
        serde_json::from_slice(
            self.app
                .project
                .authoring_document(&self.app.project.root.join(".world/en.json"))
                .unwrap()
                .bytes(),
        )
        .unwrap()
    }
    fn set_locale(&mut self, value: &Value) {
        self.app
            .project
            .set_authoring_document(
                &self.app.project.root.join(".world/en.json"),
                serde_json::to_vec(value).unwrap(),
            )
            .unwrap();
        self.app.recompile();
    }
    fn disk(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn read(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    read(root, &path, result);
                } else {
                    result.insert(
                        path.strip_prefix(root).unwrap().into(),
                        fs::read(path).unwrap(),
                    );
                }
            }
        }
        let mut result = BTreeMap::new();
        read(&self.app.project.root, &self.app.project.root, &mut result);
        result
    }
}
impl Drop for Flow {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
fn position(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text == label => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| position(shape, label)),
        _ => None,
    }
}
fn copied(output: &egui::FullOutput) -> Option<&str> {
    output
        .platform_output
        .commands
        .iter()
        .find_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.as_str()),
            _ => None,
        })
}
fn text(parts: &[LocalizationPart]) -> String {
    serde_json::to_string(parts).unwrap()
}
