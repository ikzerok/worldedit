use super::{click, collect_text, frame, Project, WorldeditApp};
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) static NEXT_TEST_ROOT: AtomicUsize = AtomicUsize::new(0);

pub(super) fn app() -> (egui::Context, WorldeditApp) {
    app_in_directory(&std::env::temp_dir())
}

fn app_in_directory(directory: &std::path::Path) -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = directory.join(format!(
        "worldedit-form-ui-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    app.project = Project::new(&root);
    let root = app.project.root.clone();
    let entry = app.project.entry.clone();
    app.project.documents.retain(|path, _| path == &entry);
    app.project.set_text(&entry, "entity a kind place as \"同名\"\nentity b kind organization as \"同名\"\nrelation_type knows as \"认识\"\n".into()).unwrap();
    app.project.create_authoring_document(&root.join(".world/project.json"), br#"{
        "schema_version":1,"language_version":"1.10",
        "required_features":["content.entities.v1","content.relations.v1"],"maps":{},"graph_views":{}
    }"#.to_vec()).unwrap();
    app.active_file = entry;
    app.reset_views();
    app.recompile();
    (ctx, app)
}

pub(super) fn register_project_template(app: &mut WorldeditApp, document: &str) {
    let parsed: serde_json::Value = serde_json::from_str(document).unwrap();
    let id = parsed["id"].as_str().unwrap();
    let slug = id.strip_prefix("project:").unwrap();
    let relative = format!(".world/templates/{slug}.json");
    let manifest_path = app.project.root.join(".world/project.json");
    let mut manifest: serde_json::Value = serde_json::from_slice(
        app.project
            .authoring_document(&manifest_path)
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["required_features"] = serde_json::json!([
        "content.entities.v1",
        "content.relations.v1",
        "content.templates.v1",
        "content.object_refs.v1"
    ]);
    manifest["templates"][id] = relative.clone().into();
    app.project
        .set_authoring_document(&manifest_path, serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    app.project
        .create_authoring_document(
            &app.project.root.join(relative),
            document.as_bytes().to_vec(),
        )
        .unwrap();
    app.recompile();
}
pub(super) fn manuscript_app() -> (egui::Context, WorldeditApp) {
    manuscript_app_in_directory(&std::env::temp_dir())
}

pub(super) fn manuscript_app_in_directory(
    directory: &std::path::Path,
) -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app_in_directory(directory);
    app.project
        .set_text(
            &app.active_file.clone(),
            concat!(
                "character traveler as \"旅人\"\n",
                "event arrival as \"抵达\"\n",
                "  甲乙 [[character:traveler|林澈]]\n",
                "  scene harbor\n",
                "    灯塔亮起。\n",
                "    -> END\n",
                "  -> END\n",
                "event departure as \"离港\"\n",
                "  远航。\n",
                "  -> END\n",
                "entity a kind place as \"同名\"\n",
                "  description \"同名地点资料。\"\n",
                "entity b kind organization as \"同名\"\n",
                "  description \"同名组织资料。\"\n",
            )
            .into(),
        )
        .unwrap();
    app.project
        .set_authoring_document(
            &app.project.root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.10","entry":"world.wl","required_features":["content.entities.v1","content.relations.v1","presentation.manuscripts.v1"],"maps":{},"graph_views":{},"manuscripts":{"novel":".world/manuscripts/novel.json"}}"#.to_vec(),
        )
        .unwrap();
    app.project
        .create_authoring_document(
            &app.project.root.join(".world/manuscripts/novel.json"),
            r#"{"schema_version":1,"id":"novel","title":"雾港书稿","entries":[{"id":"opening","kind":"chapter","title":"抵达","target_ref":{"kind":"event","id":"arrival"},"summary":"旅人来到港口","status":"draft","goal":"100"},{"id":"departure","kind":"chapter","title":"离港","target_ref":{"kind":"event","id":"departure"},"status":"planned","goal":"50"},{"id":"harbor","kind":"chapter","title":"港口","target_ref":{"kind":"scene","id":"arrival.harbor"}},{"id":"notes","kind":"chapter","title":"同名资料","target_ref":{"kind":"entity","id":"a"}}]}"#.as_bytes().to_vec(),
        )
        .unwrap();
    app.reset_views();
    app.recompile();
    (ctx, app)
}

pub(super) fn reader_publish_app() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app();
    app.project
        .set_text(
            &app.active_file.clone(),
            concat!(
                "event public as \"Public Event\"\n",
                "  Published story body.\n",
                "  [[event:private|HIDDEN_LINK_LABEL_SENTINEL]]\n",
                "  -> END\n",
                "event private as \"HIDDEN_EVENT_TITLE_SENTINEL\"\n",
                "  HIDDEN_EVENT_BODY_SENTINEL.\n",
                "  -> END\n",
                "asset cover image \"assets/cover.png\" as \"Public Cover\"\n",
                "asset hidden file \"private/secret.txt\" as \"HIDDEN_ASSET_TITLE_SENTINEL\"\n",
            )
            .into(),
        )
        .unwrap();
    app.project
        .set_authoring_document(
            &app.project.root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.10","required_features":["content.entities.v1","content.relations.v1","presentation.manuscripts.v1"],"maps":{},"graph_views":{},"manuscripts":{"public-book":".world/manuscripts/public-book.json"}}"#.to_vec(),
        )
        .unwrap();
    app.project
        .create_authoring_document(
            &app.project.root.join(".world/manuscripts/public-book.json"),
            br#"{"schema_version":1,"id":"public-book","title":"Public Book","entries":[{"id":"public-chapter","kind":"chapter","title":"Public Chapter","target_ref":{"kind":"event","id":"public"},"summary":"Public chapter summary","status":"published"},{"id":"hidden-chapter","kind":"chapter","title":"HIDDEN_CHAPTER_TITLE_SENTINEL","target_ref":{"kind":"event","id":"private"},"summary":"HIDDEN_CHAPTER_BODY_SENTINEL","status":"draft"}]}"#.to_vec(),
        )
        .unwrap();
    app.project.save().unwrap();
    std::fs::create_dir_all(app.project.root.join("assets")).unwrap();
    std::fs::create_dir_all(app.project.root.join("private")).unwrap();
    std::fs::write(app.project.root.join("assets/cover.png"), [0, 1, 255, 2]).unwrap();
    std::fs::write(
        app.project.root.join("private/secret.txt"),
        b"HIDDEN_ATTACHMENT_BYTES_SENTINEL",
    )
    .unwrap();
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.snapshot.as_ref().unwrap().result.diagnostics
    );
    assert!(
        app.project
            .manuscript_indices()
            .values()
            .all(|index| index.diagnostics.is_empty()),
        "{:?}",
        app.project
            .manuscript_indices()
            .values()
            .flat_map(|index| index.diagnostics.iter())
            .collect::<Vec<_>>()
    );
    (ctx, app)
}

pub(super) fn wait_for_reader_publish(ctx: &egui::Context, app: &mut WorldeditApp) {
    let mut last = String::new();
    for _ in 0..500 {
        let output = frame(ctx, app, Vec::new(), 26);
        last.clear();
        for shape in &output.shapes {
            collect_text(&shape.shape, &mut last);
        }
        if last.contains("发布 ZIP") || last.contains("预览失败：") {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("reader publish preview did not finish: {last}");
}

pub(super) fn assert_reader_package_resources_resolve(files: &crate::archive::Files) {
    for (page, bytes) in files {
        if page.extension().is_none_or(|extension| extension != "html") {
            continue;
        }
        let html = std::str::from_utf8(bytes).unwrap();
        assert!(
            html.starts_with("<!doctype html>"),
            "invalid reader page {page:?}"
        );
        for attribute in ["href=\"", "src=\""] {
            let mut rest = html;
            while let Some(start) = rest.find(attribute) {
                rest = &rest[start + attribute.len()..];
                let end = rest.find('"').expect("closed resource URL");
                let url = &rest[..end];
                assert!(!url.contains("://"), "reader package has remote URL {url}");
                assert!(
                    !url.starts_with('/'),
                    "reader URL must be package-relative: {url}"
                );
                let mut resolved = page
                    .parent()
                    .unwrap_or(std::path::Path::new(""))
                    .to_path_buf();
                for component in std::path::Path::new(url).components() {
                    match component {
                        std::path::Component::Normal(value) => resolved.push(value),
                        std::path::Component::ParentDir => assert!(
                            resolved.pop(),
                            "reader URL escapes package root: {page:?} -> {url}"
                        ),
                        std::path::Component::CurDir => {}
                        _ => panic!("unsafe reader URL {page:?} -> {url}"),
                    }
                }
                assert!(
                    files.contains_key(&resolved),
                    "missing reader resource {page:?} -> {url}"
                );
                rest = &rest[end + 1..];
            }
        }
    }
}

pub(super) fn replay_app(source: &str, window: u8) -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app();
    app.project
        .set_text(&app.active_file.clone(), source.into())
        .unwrap();
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.snapshot.as_ref().unwrap().result.diagnostics
    );
    app.tab = super::Tab::Play;
    click(&ctx, &mut app, window, "▶ 开始试玩");
    (ctx, app)
}
pub(super) fn wait_for_replay(ctx: &egui::Context, app: &mut WorldeditApp) {
    for _ in 0..500 {
        let _ = frame(ctx, app, Vec::new(), 20);
        if app.replay_debugger.job.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
