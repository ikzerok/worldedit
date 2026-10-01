use super::*;
use crate::archive;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{fs, path::Path};
use worldline_core::project::Project;

pub(super) fn app() -> (egui::Context, WorldeditApp) {
    static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = std::env::temp_dir()
        .join(format!(
            "export-scope-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
        ))
        .join("workspace");
    app.project = Project::new(&root);
    app.active_file = app.project.entry.clone();
    app.project
        .documents
        .retain(|path, _| path == &app.active_file);
    app.project
        .set_text(
            &app.active_file.clone(),
            "entity harbor kind place as \"海港\"\nevent start\n  applied_prose\n  -> END\n".into(),
        )
        .unwrap();
    app.project.create_authoring_document(&root.join(".world/project.json"),
        br#"{"schema_version":1,"language_version":"1.10","entry":"world.wl","required_features":["content.entities.v1","presentation.manuscripts.v1"],"maps":{},"graph_views":{},"manuscripts":{"novel":".world/manuscripts/novel.json"}}"#.to_vec()).unwrap();
    app.project.create_authoring_document(&root.join(".world/manuscripts/novel.json"),
        br#"{"schema_version":1,"id":"novel","title":"Export Book","entries":[{"id":"opening","kind":"chapter","title":"Opening","target_ref":{"kind":"event","id":"start"}}]}"#.to_vec()).unwrap();
    app.project.save().unwrap();
    fs::create_dir_all(root.join("unused")).unwrap();
    fs::write(root.join("unused/binary.dat"), b"\x00\xff\x01unreferenced").unwrap();
    fs::write(
        root.join("unknown.json"),
        br#"{"future":true,"extra":null}"#,
    )
    .unwrap();
    fs::write(root.join("README.md"), b"author instructions\r\n").unwrap();
    app.saved_location = true;
    app.recompile();
    (ctx, app)
}
pub(super) fn stage_drafts(app: &mut WorldeditApp) {
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    buffer.replace_source(buffer.source().replace("applied_prose", "unique_draft_610"));
    app.manuscript.restore_writing_buffers(&[buffer]);
    app.edit_entity(Some("harbor"));
    app.entity_editor.as_mut().unwrap().draft.display = "未应用资料".into();
}
fn draw(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| app.export_scope_dialog(ctx),
    )
}
pub(super) fn position(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text == label => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| position(shape, label)),
        _ => None,
    }
}
fn click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for _ in 0..3 {
        draw(ctx, app, vec![]);
    }
    let output = draw(ctx, app, vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| position(&shape.shape, label))
        .unwrap();
    for pressed in [true, false] {
        draw(
            ctx,
            app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}
fn destination(app: &WorldeditApp, zip: bool, suffix: &str) -> ExportDestination {
    let path = app.project.root.with_file_name(format!(
        "{}-{suffix}{}",
        app.project.root.file_name().unwrap().to_string_lossy(),
        if zip { ".zip" } else { "" }
    ));
    if zip {
        ExportDestination::Zip(path)
    } else {
        ExportDestination::Directory(path)
    }
}
fn exported_files(destination: &ExportDestination) -> archive::Files {
    match destination {
        ExportDestination::Zip(path) => archive::decode(&fs::read(path).unwrap()).unwrap(),
        ExportDestination::Directory(path) => worldline_core::file_access::workspace_files(path)
            .unwrap()
            .into_iter()
            .map(|file| {
                (
                    file.strip_prefix(path).unwrap().to_owned(),
                    fs::read(&file).unwrap(),
                )
            })
            .collect(),
    }
}
fn remove(destination: &ExportDestination) {
    match destination {
        ExportDestination::Zip(path) => {
            let _ = fs::remove_file(path);
        }
        ExportDestination::Directory(path) => {
            let _ = fs::remove_dir_all(path);
        }
    }
}

#[test]
fn directory_and_zip_require_explicit_scope_choice_preserve_drafts_and_all_files_then_reopen() {
    for zip in [false, true] {
        let (ctx, mut app) = app();
        // 已应用、尚未保存也属于快照，不得误称只导出磁盘已保存版本。
        let applied = format!(
            "{}\n// applied_unsaved\n",
            app.project.document(&app.active_file).unwrap()
        );
        app.project
            .set_text(&app.active_file.clone(), applied.clone())
            .unwrap();
        stage_drafts(&mut app);
        let before = app.project.content_baseline();
        let disk = fs::read(&app.active_file).unwrap();
        let inputs = app.unapplied_export_inputs();
        assert!(inputs
            .iter()
            .any(|i| i.kind == "实体资料" && i.source.contains("world.wl")));
        assert!(inputs
            .iter()
            .any(|i| i.kind == "书稿 / 正文草稿" && i.source.contains("world.wl")));
        let target = destination(&app, zip, "copy");
        app.request_strict_export(target.clone());
        assert!(app.export_confirmation.is_some());
        assert!(!target.path().exists());
        click(&ctx, &mut app, "取消导出");
        assert!(app.export_confirmation.is_none());
        assert!(!target.path().exists());
        app.request_strict_export(target.clone());
        click(&ctx, &mut app, "返回处理");
        assert!(app.export_confirmation.is_none());
        assert!(!target.path().exists());
        app.request_strict_export(target.clone());
        click(&ctx, &mut app, "明确继续：仅导出已应用版");
        assert!(app.export_confirmation.is_none(), "{:?}", app.io_error);
        let files = exported_files(&target);
        assert_eq!(files[Path::new("world.wl")], applied.as_bytes());
        for path in [
            "unused/binary.dat",
            "unknown.json",
            "README.md",
            ".world/manuscripts/novel.json",
        ] {
            assert_eq!(
                files[Path::new(path)],
                fs::read(app.project.root.join(path)).unwrap()
            );
        }
        assert_eq!(app.project.content_baseline(), before);
        assert!(app.project.is_dirty());
        assert_eq!(fs::read(&app.active_file).unwrap(), disk);
        assert!(app
            .manuscript
            .writing_buffers()
            .iter()
            .any(|b| b.source().contains("unique_draft_610")));
        assert_eq!(
            app.entity_editor.as_ref().unwrap().draft.display,
            "未应用资料"
        );
        let reopen = app.project.root.with_file_name(format!(
            "{}-reopen",
            app.project.root.file_name().unwrap().to_string_lossy()
        ));
        for (path, bytes) in files {
            let path = reopen.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        let mut reopened = Project::open(&reopen).unwrap();
        assert!(!reopened.compile().has_errors());
        assert_eq!(reopened.document(&reopened.entry).unwrap(), applied);
        let buffer = app.manuscript.writing_buffers().remove(0);
        app.project.apply_source_writing_buffer(&buffer).unwrap();
        app.manuscript
            .clear_applied_writing_buffers(&[app.active_file.clone()]);
        app.entity_editor = None;
        let applied_target = destination(&app, zip, "applied");
        app.request_strict_export(applied_target.clone());
        assert!(app.export_confirmation.is_none());
        assert!(
            String::from_utf8(exported_files(&applied_target)[Path::new("world.wl")].clone())
                .unwrap()
                .contains("unique_draft_610")
        );
        remove(&target);
        remove(&applied_target);
        let _ = fs::remove_dir_all(reopen);
        let _ = fs::remove_dir_all(app.project.root);
    }
}

#[test]
fn both_formats_preserve_error_target_boundary_and_conflict_rescue_semantics() {
    for zip in [false, true] {
        let (ctx, mut app) = app();
        stage_drafts(&mut app);
        let inside = if zip {
            ExportDestination::Zip(app.project.root.join("inside.zip"))
        } else {
            ExportDestination::Directory(app.project.root.join("inside"))
        };
        app.request_strict_export(inside.clone());
        click(&ctx, &mut app, "明确继续：仅导出已应用版");
        assert!(app.io_error.is_some());
        assert!(!inside.path().exists());
        app.export_confirmation = None;
        let existing = destination(&app, zip, "existing");
        if zip {
            fs::write(existing.path(), b"keep").unwrap();
        } else {
            fs::create_dir(existing.path()).unwrap();
        }
        app.request_strict_export(existing.clone());
        click(&ctx, &mut app, "明确继续：仅导出已应用版");
        assert!(app.io_error.as_ref().unwrap().contains("已存在"));
        if zip {
            assert_eq!(fs::read(existing.path()).unwrap(), b"keep");
        }
        app.export_confirmation = None;
        let valid = app.project.document(&app.active_file).unwrap().to_owned();
        app.project
            .set_text(&app.active_file.clone(), "event bad\n  -> missing\n".into())
            .unwrap();
        let invalid = destination(&app, zip, "invalid");
        app.request_strict_export(invalid.clone());
        click(&ctx, &mut app, "明确继续：仅导出已应用版");
        assert!(app.io_error.as_ref().unwrap().contains("编译错误"));
        assert!(!invalid.path().exists());
        app.export_confirmation = None;
        let local = format!("{valid}\n// local_buffer\n");
        app.project
            .set_text(&app.active_file.clone(), local.clone())
            .unwrap();
        fs::write(&app.active_file, format!("{valid}\n// disk_change\n")).unwrap();
        assert!(!app.project.refresh().unwrap().is_empty());
        let rescue = destination(&app, zip, "rescue");
        app.request_strict_export(rescue.clone());
        click(&ctx, &mut app, "明确继续：仅导出已应用版");
        assert_eq!(
            exported_files(&rescue)[Path::new("world.wl")],
            local.as_bytes()
        );
        assert!(fs::read_to_string(&app.active_file)
            .unwrap()
            .contains("disk_change"));
        assert!(app.project.save().is_err());
        remove(&existing);
        remove(&rescue);
        let _ = fs::remove_dir_all(app.project.root);
    }
}

#[test]
fn inventory_includes_hidden_schema_templates_multiple_sources_and_other_authoring_inputs() {
    let (ctx, mut app) = app();
    let manifest = app.project.root.join(".world/project.json");
    let manifest_bytes = app
        .project
        .authoring_document(&manifest)
        .unwrap()
        .bytes()
        .to_vec();
    app.project
        .set_authoring_document(
            &manifest,
            String::from_utf8(manifest_bytes)
                .unwrap()
                .replace("1.10", "1.12")
                .into_bytes(),
        )
        .unwrap();
    app.recompile();
    app.open_schema_editor(&ctx);
    let schema_frame = |app: &mut WorldeditApp, events| {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1500.0, 1200.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| app.schema_editor_window(ctx),
        );
    };
    schema_frame(&mut app, vec![]);
    let id = egui::Id::new("schema-source-editor");
    ctx.memory_mut(|m| m.request_focus(id));
    schema_frame(
        &mut app,
        vec![egui::Event::Text("// retained schema draft\n".into())],
    );
    assert!(app.schema_ui.has_unsubmitted_work());
    app.schema_ui.close(&ctx);
    assert!(!app.schema_ui.open);
    let template_frame = |app: &mut WorldeditApp, events| {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1500.0, 1200.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| app.template_manager_tab(ctx),
        );
    };
    template_frame(&mut app, vec![]);
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("template-manager-json")));
    template_frame(
        &mut app,
        vec![egui::Event::Text("{\"unapplied_template\":true}".into())],
    );
    let second = app.project.add_file(Path::new("notes/second.wl")).unwrap();
    app.project
        .set_text(&second, "event second\n  second applied\n  -> END\n".into())
        .unwrap();
    stage_drafts(&mut app);
    let mut buffer = app.project.open_source_writing_buffer(&second).unwrap();
    buffer.replace_source(buffer.source().replace("second applied", "second draft"));
    app.manuscript.restore_writing_buffers(&[buffer]);
    app.localization_ui.exchange_json = "pending translation JSON".into();
    app.map_selection = Some("map_1".into());
    app.map_form.annotation = "pending map note".into();
    app.ime_source_draft = Some((second.clone(), "old source".into(), "pending source".into()));
    app.review.reason = "pending proposal".into();
    app.network_view_id = "draft_layout".into();
    app.markdown_import_wizard = Some(super::super::markdown_import_ui::Wizard::default());
    app.markdown_import_wizard
        .as_mut()
        .unwrap()
        .set_source_files(
            [(PathBuf::from("note.md"), b"# Pending note".to_vec())]
                .into_iter()
                .collect(),
        );
    let baseline = app.project.content_baseline();
    let inputs = app.unapplied_export_inputs();
    for kind in [
        "持续资料约束草稿",
        "工程模板草稿",
        "书稿 / 正文草稿",
        "实体资料",
        "本地化草稿",
        "地图草稿",
        "正在输入的源码 / 输入法",
        "审阅提案",
        "共享网络布局",
        "Markdown 导入草稿",
    ] {
        assert!(
            inputs
                .iter()
                .any(|input| input.kind == kind && !input.source.is_empty()),
            "缺少 {kind}: {inputs:?}"
        );
    }
    assert_eq!(
        inputs
            .iter()
            .filter(|i| i.kind == "书稿 / 正文草稿")
            .count(),
        2
    );
    assert!(inputs
        .iter()
        .any(|i| i.kind == "持续资料约束草稿" && i.source.ends_with("world.wl")));
    assert!(inputs.iter().any(|i| i.kind == "正在输入的源码 / 输入法"
        && i.source.replace('\\', "/").ends_with("notes/second.wl")));
    assert_eq!(app.project.content_baseline(), baseline);
    let _ = fs::remove_dir_all(app.project.root);
}

#[test]
fn clean_forms_do_not_prompt_and_workspace_switch_cannot_reuse_export_confirmation() {
    let (_ctx, mut app) = app();
    app.edit_entity(Some("harbor"));
    assert!(app.unapplied_export_inputs().is_empty());
    let target = destination(&app, false, "clean");
    app.request_strict_export(target.clone());
    assert!(app.export_confirmation.is_none());
    assert!(target.path().exists());
    stage_drafts(&mut app);
    app.request_strict_export(destination(&app, false, "never-write"));
    assert!(app.export_confirmation.is_some());
    app.reset_views();
    assert!(app.export_confirmation.is_none());
    remove(&target);
    let _ = fs::remove_dir_all(app.project.root);
}

#[test]
fn changing_only_new_form_source_path_is_an_unapplied_input() {
    let (_ctx, mut app) = app();
    let second = app.project.add_file(Path::new("notes/second.wl")).unwrap();
    app.recompile();
    app.new_event(None);
    assert!(!app
        .unapplied_export_inputs()
        .iter()
        .any(|i| i.kind == "事件正文与分支"));
    app.event_editor.as_mut().unwrap().path = second.clone();
    assert!(app
        .unapplied_export_inputs()
        .iter()
        .any(|i| i.kind == "事件正文与分支"
            && i.source.replace('\\', "/").contains("notes/second.wl")));
    app.new_character();
    assert!(!app
        .unapplied_export_inputs()
        .iter()
        .any(|i| i.kind == "人物资料"));
    app.character_editor.as_mut().unwrap().path = second;
    assert!(app
        .unapplied_export_inputs()
        .iter()
        .any(|i| i.kind == "人物资料" && i.source.replace('\\', "/").contains("notes/second.wl")));
    let _ = fs::remove_dir_all(app.project.root);
}

#[test]
fn clearing_loaded_template_to_empty_still_requires_export_scope_confirmation() {
    let (ctx, mut app) = app();
    assert!(!app
        .unapplied_export_inputs()
        .iter()
        .any(|i| i.kind == "工程模板草稿"));
    let manifest_path = app.project.root.join(".world/project.json");
    let mut manifest: serde_json::Value = serde_json::from_slice(
        app.project
            .authoring_document(&manifest_path)
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["required_features"]
        .as_array_mut()
        .unwrap()
        .push("content.templates.v1".into());
    manifest["templates"] = serde_json::json!({"project:typed":".world/templates/typed.json"});
    app.project
        .set_authoring_document(&manifest_path, serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    let template_path = app.project.root.join(".world/templates/typed.json");
    let original = br#"{"schema_version":1,"id":"project:typed","title":"Typed fields","applies_to":{"kind":"entity","entity_type":"place"},"fields":[{"id":"memo","key":"memo","label":"Memo","type":"text","required":false}]}"#;
    app.project
        .create_authoring_document(&template_path, original.to_vec())
        .unwrap();
    app.recompile();
    let template_frame = |app: &mut WorldeditApp, events| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1500.0, 1200.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| app.template_manager_tab(ctx),
        )
    };
    for _ in 0..3 {
        template_frame(&mut app, vec![]);
    }
    let output = template_frame(&mut app, vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| position(&shape.shape, "Typed fields · project:typed"))
        .unwrap();
    for pressed in [true, false] {
        template_frame(
            &mut app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert!(!app
        .unapplied_export_inputs()
        .iter()
        .any(|i| i.kind == "工程模板草稿"));
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("template-manager-json")));
    for (key, modifiers) in [
        (egui::Key::A, egui::Modifiers::COMMAND),
        (egui::Key::Backspace, egui::Modifiers::NONE),
    ] {
        template_frame(
            &mut app,
            vec![egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers,
            }],
        );
    }
    assert!(app
        .unapplied_export_inputs()
        .iter()
        .any(|i| i.kind == "工程模板草稿" && i.source.contains("project:typed")));
    assert_eq!(
        app.project
            .authoring_document(&template_path)
            .unwrap()
            .bytes(),
        original
    );
    let target = destination(&app, false, "empty-template");
    app.request_strict_export(target.clone());
    assert!(app.export_confirmation.is_some());
    assert!(!target.path().exists());
    click(&ctx, &mut app, "取消导出");
    assert!(app
        .unapplied_export_inputs()
        .iter()
        .any(|i| i.kind == "工程模板草稿"));
    let _ = fs::remove_dir_all(app.project.root);
}
