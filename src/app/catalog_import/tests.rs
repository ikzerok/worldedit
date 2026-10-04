//! 合成 egui 输入与真实 core 项目；不代替物理 IME 或浏览器验收。
use super::*;
use egui::{Event, Key, PointerButton, Pos2, RawInput, Rect, Vec2};
use std::sync::atomic::{AtomicUsize, Ordering};
use worldline_core::catalog_import::{
    CatalogBlankPolicy as Blank, CatalogImportField as Field, CatalogImportType as Type,
};
use worldline_core::project::Project;

mod interaction;
mod layout;
mod lifecycle;
mod native_path;
mod paths;
mod safety;
mod snapshot;
use interaction::*;

const CSV: &str = "kind,id,display,age,notes\ncharacter,traveler,远行旅人,21,明确忽略的原始列\ncharacter,guide,向导,35,另一个备注\n";

fn app() -> (egui::Context, WorldeditApp) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let ctx = egui::Context::default();
    ctx.style_mut(|s| s.animation_time = 0.0);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = std::env::temp_dir().join(format!(
        "catalog-import-ui-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    app.project = Project::new(&root);
    let entry = app.project.entry.clone();
    app.project.documents.retain(|path, _| path == &entry);
    app.project.set_text(&entry, "character traveler as \"旅人\"\n  property age = 20\n  // 不在导入范围的备注\n  property voice = \"轻声\"\nevent opening as \"开场\"\n  -> END\n".into()).unwrap();
    app.active_file = entry;
    app.reset_views();
    app.recompile();
    app.tab = Tab::CatalogImport;
    (ctx, app)
}

fn load(ctx: &egui::Context, app: &mut WorldeditApp, csv: &str) {
    app.catalog_import
        .offer_file("资料.csv".into(), csv.as_bytes().to_vec(), ctx)
        .unwrap();
    wait(ctx, app);
    assert!(
        app.catalog_import.table.is_some(),
        "{:?}",
        app.catalog_import.error
    );
}
fn map(app: &mut WorldeditApp) {
    let state = &mut app.catalog_import;
    state.columns = [
        Field::Kind,
        Field::Id,
        Field::Display,
        Field::Property {
            key: "age".into(),
            value_type: Type::Number,
        },
        Field::Ignore,
    ]
    .into_iter()
    .enumerate()
    .map(|(column, field)| {
        Some(CatalogColumnMapping {
            column,
            field,
            blank: Blank::Error,
        })
    })
    .collect();
    state.destination = app
        .project
        .entry
        .strip_prefix(&app.project.root)
        .unwrap()
        .to_path_buf();
    state.invalidate();
}
fn preview(ctx: &egui::Context, app: &mut WorldeditApp) {
    let mut state = std::mem::take(&mut app.catalog_import);
    state.preview(app, ctx);
    app.catalog_import = state;
    wait(ctx, app);
}
fn apply(app: &mut WorldeditApp) {
    let mut state = std::mem::take(&mut app.catalog_import);
    state.acknowledged = true;
    state.apply(app);
    app.catalog_import = state;
}
fn wait(ctx: &egui::Context, app: &mut WorldeditApp) {
    for _ in 0..1000 {
        let mut state = std::mem::take(&mut app.catalog_import);
        state.poll(app, ctx);
        app.catalog_import = state;
        if app.catalog_import.job.is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("导入后台检查未完成");
}
