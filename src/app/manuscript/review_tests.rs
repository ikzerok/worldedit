//! 离屏 egui 交互与真实 Project 来源守卫；不替代原生窗口验收。
use super::*;
use crate::app::{Tab, WorldeditApp};
use egui::{pos2, vec2, Event, RawInput, Rect};
use std::sync::atomic::{AtomicUsize, Ordering};
use worldline_core::project::Project;
mod navigation;
mod layout;

const SOURCE: &str = concat!(
    "let public = true\nlet rescue = false\ncharacter lin as \"林芜\"\n",
    "fragment signal()\n  say lin \"片段内隐藏正文\"\n  return\n",
    "event start as \"退潮\"\n",
    "  灯塔亮起，[[character:lin|林芜]]带着档案走过长长的石阶。\n",
    "  if public\n    if rescue\n      公开营救结局。\n    else\n      公开留守结局。\n",
    "  else\n    if rescue\n      隐瞒营救结局。\n    else\n      隐瞒留守结局。\n",
    "  choice once \"公开档案\" if public enable rescue disabled \"燃料不足\"\n    say lin \"请记住这座城。\" direction \"低声\"\n    call signal()\n",
    "  choice \"带回档案\"\n    -> END\n",
    "  scene dawn\n    天将破晓。\n  -> END\n",
    "event second as \"第二章\"\n  第二章独有正文。\n  -> END\n",
);

fn app_with_source(source: &str) -> (egui::Context, WorldeditApp) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = std::env::temp_dir().join(format!("worldedit-branch-review-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    app.project = Project::new(&root);
    let path = app.project.entry.clone();
    app.project.documents.retain(|entry, _| entry == &path);
    app.project.set_text(&path, source.into()).unwrap();
    app.project.create_authoring_document(&root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.11","entry":"world.wl","required_features":["presentation.manuscripts.v1"],"manuscripts":{"book":".world/manuscripts/book.json"}}"#.to_vec()).unwrap();
    app.project.create_authoring_document(&root.join(".world/manuscripts/book.json"), br#"{"schema_version":1,"id":"book","title":"Review Book","entries":[{"id":"first","kind":"chapter","title":"First","target_ref":{"kind":"event","id":"start"}},{"id":"second","kind":"chapter","title":"Second","target_ref":{"kind":"event","id":"second"}}]}"#.to_vec()).unwrap();
    app.active_file = path;
    app.reset_views();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors(), "{:?}", app.snapshot.as_ref().unwrap().result.diagnostics);
    app.tab = Tab::Manuscript;
    (ctx, app)
}

fn frame(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2, events: Vec<Event>) -> egui::FullOutput {
    ctx.run(RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)), events, ..Default::default() }, |ctx| {
        app.author_shortcuts(ctx);
        app.top_bar(ctx);
        app.status_bar(ctx);
        app.sidebar(ctx);
        app.manuscript_tab(ctx);
    })
}

fn settle(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2) -> egui::FullOutput {
    for _ in 0..3 { frame(ctx, app, size, vec![]); }
    frame(ctx, app, size, vec![])
}

fn text_shapes<'a>(shape: &'a egui::Shape, out: &mut Vec<&'a egui::epaint::TextShape>) {
    match shape {
        egui::Shape::Text(text) => out.push(text),
        egui::Shape::Vec(shapes) => for shape in shapes { text_shapes(shape, out); },
        _ => {}
    }
}

fn texts(output: &egui::FullOutput) -> String {
    output.shapes.iter().flat_map(|clipped| { let mut out = Vec::new(); text_shapes(&clipped.shape, &mut out); out }).map(|text| text.galley.job.text.as_str()).collect::<Vec<_>>().join("\n")
}

fn visible(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
    for clipped in &output.shapes {
        let mut shapes = Vec::new(); text_shapes(&clipped.shape, &mut shapes);
        for text in shapes {
            if text.galley.job.text == label {
                let point = text.pos + text.galley.rect.center().to_vec2();
                if clipped.clip_rect.contains(point) { return Some(point); }
            }
        }
    }
    None
}

fn click(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2, label: &str) {
    let output = settle(ctx, app, size);
    let pos = visible(&output, label).unwrap_or_else(|| panic!("missing visible {label}: {}", texts(&output)));
    for pressed in [true, false] {
        frame(ctx, app, size, vec![Event::PointerMoved(pos), Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }]);
    }
}

fn key(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2, key: egui::Key, modifiers: egui::Modifiers) -> egui::FullOutput {
    frame(ctx, app, size, vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }]);
    frame(ctx, app, size, vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers }])
}

fn request(app: &WorldeditApp, needle: &str) -> review_navigation::ReviewRequest {
    fn source<'a>(nodes: &'a [worldline_core::manuscript::ReviewNode], needle: &str) -> Option<&'a worldline_core::manuscript::ReviewSource> {
        for node in nodes {
            if let Some(source) = node.source.as_ref().filter(|source| source.excerpt.contains(needle)) { return Some(source); }
            if let Some(source) = source(&node.children, needle) { return Some(source); }
        }
        None
    }
    let review = app.manuscript.preview_cache.current.get(&TargetRef::new("event", "start")).unwrap().clone();
    review_navigation::ReviewRequest { key: app.manuscript.preview_cache.key.clone(), source: source(&review.nodes, needle).unwrap().clone(), review }
}
