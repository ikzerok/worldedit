use crate::app::WorldeditApp;
use egui::{Event, Key, Modifiers, PointerButton};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use worldline_core::project::Project;

#[derive(Default)]
pub(super) struct Storage(pub(super) BTreeMap<String, String>);
impl eframe::Storage for Storage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.into(), value);
    }
    fn flush(&mut self) {}
}

pub(super) struct Workspace(pub(super) PathBuf);
impl Workspace {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "worldedit-projectless-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    pub(super) fn empty(&self, name: &str) -> PathBuf {
        let root = self.0.join(name);
        std::fs::create_dir(&root).unwrap();
        root
    }
    pub(super) fn project(&self, name: &str) -> PathBuf {
        let root = self.empty(name);
        Project::new(&root).save().unwrap();
        root
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn app(path: Option<PathBuf>) -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::start_native(&creation, path);
    app.personal.settings.appearance.reduce_motion = true;
    (ctx, app)
}

pub(super) fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 720.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest()),
    )
}

fn text(shape: &egui::Shape, output: &mut String) {
    match shape {
        egui::Shape::Text(shape) => output.push_str(&shape.galley.job.text),
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| text(shape, output)),
        _ => {}
    }
}
pub(super) fn rendered(output: &egui::FullOutput) -> String {
    let mut result = String::new();
    for shape in &output.shapes {
        text(&shape.shape, &mut result);
    }
    result
}
pub(super) fn position(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(shape) if shape.galley.job.text == label => {
            Some(shape.pos + shape.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().rev().find_map(|shape| position(shape, label)),
        _ => None,
    }
}
pub(super) fn click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for _ in 0..3 {
        frame(ctx, app, vec![]);
    }
    let output = frame(ctx, app, vec![]);
    let point = output
        .shapes
        .iter()
        .rev()
        .find_map(|shape| position(&shape.shape, label).filter(|p| shape.clip_rect.contains(*p)))
        .unwrap_or_else(|| panic!("未显示按钮 {label}: {}", rendered(&output)));
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
}
pub(super) fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}
pub(super) fn enter_path(ctx: &egui::Context, app: &mut WorldeditApp, path: &Path) {
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("workspace-startup-path")));
    frame(
        ctx,
        app,
        vec![
            key(Key::A, Modifiers::COMMAND),
            Event::Text(path.to_string_lossy().into_owned()),
        ],
    );
    assert_eq!(
        app.native_startup.dialog.as_ref().unwrap().path,
        path.to_string_lossy()
    );
}
pub(super) fn has_close(output: &egui::FullOutput) -> bool {
    output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .iter()
        .any(|command| matches!(command, egui::ViewportCommand::Close))
}
pub(super) fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files.insert(path.strip_prefix(root).unwrap().into(), Vec::new());
                visit(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().into(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}
