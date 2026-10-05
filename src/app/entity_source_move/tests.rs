use super::*;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use worldline_core::project::Project;
mod full_app;
mod keyboard;
const SOURCE: &str = "设定/旧灯塔与其他资料.wl";
const TARGET: &str = "资料/长中文目录用于检验完整路径的可读性/地点资料与档案目标.wl";
const DECLARATION: &str = "entity lighthouse kind place as \"北雾灯塔🙂\"\n  description \"海边旧灯\"\n  property lit = true\n";
const SOURCE_TEXT: &str = "// 原稿前言\nentity lighthouse kind place as \"北雾灯塔🙂\"\n  description \"海边旧灯\"\n  property lit = true\n\n// 守灯会资料留在原稿\nentity keepers kind organization as \"守灯会\"\n";
const TARGET_TEXT: &str = "// 已存在的活动目标，末尾没有换行";
fn fixture() -> (egui::Context, WorldeditApp) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "entity-move-ui-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    for (path, text) in [
        (SOURCE, SOURCE_TEXT), (TARGET, TARGET_TEXT),
        ("world.wl", "event start\n  [[entity:lighthouse|灯塔]]\n  -> END\nstate lamp on entity lighthouse with [] as \"灯光\"\n"),
        ("archive.wl", "entity archive kind place\n"),
        ("inactive.wl", "entity inactive kind place\n"),
        (".hidden/unused.txt", "未引用文件完整保留🙂"),
    ] {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fs::create_dir_all(root.join(".world")).unwrap();
    fs::write(root.join(".world/project.json"), serde_json::json!({
        "schema_version":1, "language_version":"1.13", "entry":"world.wl",
        "required_features":["workspace.source_sets.v1", "content.entities.v1"],
        "source_config":{"mode":"explicit", "active":["world.wl", SOURCE, TARGET], "archived":["archive.wl"]}
    }).to_string()).unwrap();
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    app.project = Project::open(&root).unwrap();
    app.active_file = app.project.root.join(SOURCE);
    app.reset_views();
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.diagnostics()
    );
    (ctx, app)
}
fn planned(app: &mut WorldeditApp) -> EntitySourceMoveForm {
    app.begin_entity_source_move("lighthouse");
    let mut form = app.entity_source_move_form.take().unwrap();
    form.select(app.project.root.join(TARGET));
    app.preview_entity_source_move(&mut form);
    assert!(form.error.is_none(), "{:?}", form.error);
    assert!(form.plan.is_some());
    form
}
fn assert_source(app: &WorldeditApp, relative: &str) {
    let object = app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .object(&TargetRef::new("entity", "lighthouse"))
        .unwrap();
    assert_eq!(PathBuf::from(&object.file), app.project.root.join(relative));
    assert_eq!(app.active_file, app.project.root.join(relative));
    assert_eq!(app.jump, Some((object.line, 1)));
}
#[test]
fn entity_move_preview_apply_undo_redo_save_reopen_follow_stable_identity() {
    let (ctx, mut app) = fixture();
    let root = app.project.root.clone();
    let before = app.project.sources();
    let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
    let unrelated = super::super::personal::Location {
        file: root.join(SOURCE),
        target: Some(TargetRef::new("entity", "keepers")),
        ..Default::default()
    };
    app.personal.history.push(unrelated.clone());
    app.reading_return = Some((root.join(SOURCE), 7));
    let mut form = planned(&mut app);
    let plan = form.plan.as_ref().unwrap();
    assert_eq!(plan.changes.len(), 2);
    assert_eq!(app.project.sources(), before);
    assert!(app.history.is_empty());
    assert!(plan
        .changes
        .iter()
        .flat_map(|change| &change.occurrences)
        .any(|occurrence| occurrence.before_token == DECLARATION));
    assert!(app.apply_entity_source_move(&mut form), "{:?}", form.error);
    assert_eq!(app.history.len(), 1);
    assert_source(&app, TARGET);
    assert!(app.personal.history[0] == unrelated);
    assert_eq!(app.reading_return, Some((root.join(SOURCE), 7)));
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
    assert_eq!(
        fs::read_to_string(root.join(SOURCE)).unwrap(),
        SOURCE_TEXT,
        "应用不自动保存"
    );
    assert!(app.message.as_ref().unwrap().contains("尚未保存"));
    assert_eq!(
        before
            .iter()
            .filter(|(path, text)| app.project.document(path).unwrap() != text.as_str())
            .count(),
        2
    );
    let moved_location = app.author_location(None);
    app.undo(false);
    assert_eq!(app.project.sources(), before);
    assert_source(&app, SOURCE);
    app.restore_author_location(moved_location, &ctx);
    assert_source(&app, SOURCE);
    app.undo(true);
    assert_source(&app, TARGET);
    assert!(app
        .project
        .document(&root.join(SOURCE))
        .unwrap()
        .contains("entity keepers"));
    app.project.save().unwrap();
    assert!(!app.project.is_dirty());
    app.project = Project::open(&root).unwrap();
    app.recompile();
    app.jump_to_entity_source("lighthouse");
    assert_source(&app, TARGET);
    assert_eq!(
        fs::read_to_string(root.join(".hidden/unused.txt")).unwrap(),
        "未引用文件完整保留🙂"
    );
    let _ = fs::remove_dir_all(root);
}
#[test]
fn entity_move_same_source_is_zero_change_without_undo_or_dirty_state() {
    let (_, mut app) = fixture();
    let baseline = app.project.content_baseline();
    app.begin_entity_source_move("lighthouse");
    let mut form = app.entity_source_move_form.take().unwrap();
    app.preview_entity_source_move(&mut form);
    assert!(form.plan.as_ref().unwrap().changes.is_empty());
    assert!(app.apply_entity_source_move(&mut form));
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    assert!(!app.project.is_dirty());
    assert!(app.message.as_ref().unwrap().contains("没有修改"));
    let _ = fs::remove_dir_all(app.project.root);
}
#[test]
fn entity_move_blocks_dirty_form_and_body_without_replacing_them() {
    let (_, mut app) = fixture();
    let baseline = app.project.content_baseline();
    app.edit_entity(Some("lighthouse"));
    app.entity_editor.as_mut().unwrap().draft.description = "尚未应用的灯塔新说明".into();
    app.begin_current_entity_source_move();
    assert!(app.entity_source_move_form.is_none());
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.description,
        "尚未应用的灯塔新说明"
    );
    app.entity_editor = None;
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.project.entry)
        .unwrap();
    buffer.replace_source(buffer.source().replace("灯塔", "正文草稿完整保留"));
    app.manuscript.restore_writing_buffers(&[buffer]);
    app.begin_entity_source_move("lighthouse");
    assert!(app.entity_source_move_form.is_none());
    assert!(app.manuscript.writing_buffers()[0]
        .source()
        .contains("正文草稿完整保留"));
    assert_eq!(app.project.content_baseline(), baseline);
    let _ = fs::remove_dir_all(app.project.root);
}
#[test]
fn entity_move_targets_are_active_existing_and_selection_is_export_guarded() {
    let (_, mut app) = fixture();
    let targets = app.entity_move_targets();
    assert_eq!(targets.len(), 3);
    assert!(!targets.contains(&app.project.root.join("archive.wl")));
    assert!(!targets.contains(&app.project.root.join("inactive.wl")));
    let mut form = planned(&mut app);
    form.select(app.project.entry.clone());
    assert!(form.plan.is_none());
    app.entity_source_move_form = Some(form);
    assert!(app.dirty_draft_names().contains(&"实体移源"));
    assert!(app
        .unapplied_export_inputs()
        .iter()
        .any(|input| input.kind == "实体移源" && input.source.contains("lighthouse")));
    let baseline = app.project.content_baseline();
    app.discard_authoring_drafts();
    assert!(app.entity_source_move_form.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    let _ = fs::remove_dir_all(app.project.root);
}
#[test]
fn entity_move_rejects_changed_plan_disk_inventory_and_new_draft() {
    for mutation in [
        "source",
        "target",
        "other",
        "inventory",
        "disk",
        "tamper",
        "draft",
        "capability",
        "target_deleted",
    ] {
        let (_, mut app) = fixture();
        let root = app.project.root.clone();
        let mut form = planned(&mut app);
        match mutation {
            "source" | "target" | "other" => {
                let path = root.join(match mutation {
                    "source" => SOURCE,
                    "target" => TARGET,
                    _ => "world.wl",
                });
                app.project
                    .set_text(
                        &path,
                        format!("{}\n// 新的当前修改", app.project.document(&path).unwrap()),
                    )
                    .unwrap();
            }
            "inventory" => {
                fs::write(root.join("new.wl"), "// 新库存\n").unwrap();
            }
            "disk" => {
                fs::write(root.join(TARGET), "// 盘上外部更新\n").unwrap();
            }
            "capability" => {
                let path = root.join(".world/project.json");
                let mut manifest: serde_json::Value =
                    serde_json::from_slice(app.project.authoring_document(&path).unwrap().bytes())
                        .unwrap();
                manifest["required_features"]
                    .as_array_mut()
                    .unwrap()
                    .push("future.blocked.v1".into());
                // 未知必需能力不能通过编辑 API 写入；真实外部变更由 refresh 导入只读状态。
                let bytes = serde_json::to_vec(&manifest).unwrap();
                fs::write(&path, &bytes).unwrap();
                assert!(app.project.refresh().unwrap().is_empty());
                assert!(!app.project.authoring_diagnostics().is_empty());
                assert_eq!(
                    app.project.authoring_document(&path).unwrap().bytes(),
                    bytes
                );
            }
            "target_deleted" => {
                app.project.documents.remove(&root.join(TARGET));
            }
            "tamper" => {
                form.plan.as_mut().unwrap().changes[0].occurrences[0]
                    .before_token
                    .push_str("篡改");
            }
            "draft" => {
                app.edit_entity(Some("lighthouse"));
                app.entity_editor.as_mut().unwrap().draft.display = "新草稿".into();
            }
            _ => unreachable!(),
        }
        let baseline = app.project.content_baseline();
        let sources = app.project.sources();
        let disk_before: Vec<_> = [SOURCE, TARGET, ".world/project.json"]
            .into_iter()
            .map(|relative| (root.join(relative), fs::read(root.join(relative)).unwrap()))
            .collect();
        assert!(
            !app.apply_entity_source_move(&mut form),
            "{mutation} must refuse"
        );
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(app.project.sources(), sources);
        for (path, bytes) in disk_before {
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
        assert!(app.history.is_empty());
        assert_eq!(form.destination, root.join(TARGET));
        assert!(form.error.is_some());
        if mutation == "draft" {
            assert_eq!(app.entity_editor.as_ref().unwrap().draft.display, "新草稿");
        }
        let _ = fs::remove_dir_all(root);
    }
}
