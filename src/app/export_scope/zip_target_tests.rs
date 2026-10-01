//! 真实 egui 路径输入、目标决定点和范围确认链；不调用系统文件选择器。
use super::{
    tests::{app, position, stage_drafts},
    *,
};
use std::{fs, path::Path};

fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 960.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            app.author_shortcuts(ctx);
            app.dialogs(ctx);
            app.export_scope_dialog(ctx);
            app.capture_edit_focus(ctx);
        },
    )
}
fn click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for _ in 0..3 {
        frame(ctx, app, vec![]);
    }
    let output = frame(ctx, app, vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| position(&shape.shape, label))
        .unwrap_or_else(|| panic!("未显示按钮：{label}"));
    for pressed in [true, false] {
        frame(
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
fn enter_path(ctx: &egui::Context, app: &mut WorldeditApp, text: &str) {
    frame(ctx, app, vec![]);
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("export-target-path")));
    frame(
        ctx,
        app,
        vec![egui::Event::Key {
            key: egui::Key::A,
            physical_key: Some(egui::Key::A),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
    );
    frame(ctx, app, vec![egui::Event::Text(text.into())]);
    assert_eq!(app.directory.as_ref().unwrap().path, text);
}
fn zip_target(app: &WorldeditApp, suffix: &str) -> PathBuf {
    app.project
        .root
        .with_file_name(format!("target-{suffix}.zip"))
}

#[test]
fn zip_target_then_scope_cancel_change_target_and_continue_preserve_drafts() {
    let (ctx, mut app) = app();
    stage_drafts(&mut app);
    let baseline = app.project.content_baseline();
    let dirty = app.project.is_dirty();
    let disk = fs::read(&app.active_file).unwrap();
    app.export_package();
    let suggested = PathBuf::from(&app.directory.as_ref().unwrap().path);
    assert!(suggested.is_absolute() && !suggested.exists());
    assert!(app.export_confirmation.is_none());
    let first = zip_target(&app, "cancelled");
    enter_path(&ctx, &mut app, first.to_str().unwrap());
    app.directory
        .as_mut()
        .unwrap()
        .accept_native_selection(None);
    assert_eq!(
        app.directory.as_ref().unwrap().path,
        first.to_str().unwrap()
    );
    assert!(!first.exists());
    click(&ctx, &mut app, "取消");
    assert!(app.directory.is_none() && app.export_confirmation.is_none());
    assert!(!first.exists());
    app.export_package();
    enter_path(&ctx, &mut app, first.to_str().unwrap());
    click(&ctx, &mut app, "校验并导出");
    assert!(app.directory.is_none() && app.export_confirmation.is_some());
    assert!(!first.exists());
    click(&ctx, &mut app, "取消导出");
    assert!(app.export_confirmation.is_none());
    assert!(!first.exists());
    app.export_package();
    enter_path(&ctx, &mut app, first.to_str().unwrap());
    click(&ctx, &mut app, "校验并导出");
    click(&ctx, &mut app, "修改目标路径");
    assert!(app.export_confirmation.is_none());
    assert_eq!(
        app.directory.as_ref().unwrap().path,
        first.to_str().unwrap()
    );
    let final_target = zip_target(&app, "confirmed");
    enter_path(&ctx, &mut app, final_target.to_str().unwrap());
    click(&ctx, &mut app, "校验并导出");
    assert!(!final_target.exists());
    click(&ctx, &mut app, "明确继续：仅导出已应用版");
    assert!(app.export_confirmation.is_none(), "{:?}", app.io_error);
    let files = crate::archive::decode(&fs::read(&final_target).unwrap()).unwrap();
    assert_eq!(files[Path::new("world.wl")], disk);
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
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.is_dirty(), dirty);
    assert_eq!(fs::read(&app.active_file).unwrap(), disk);
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.display,
        "未应用资料"
    );
    assert!(app
        .manuscript
        .writing_buffers()
        .iter()
        .any(|b| b.source().contains("unique_draft_610")));
    assert!(!first.exists() && !suggested.exists());
    let _ = fs::remove_dir_all(app.project.root.parent().unwrap());
}

#[test]
fn zip_target_rejects_relative_existing_inside_and_invalid_core_then_allows_correction() {
    let (ctx, mut app) = app();
    app.export_package();
    enter_path(&ctx, &mut app, "relative.zip");
    click(&ctx, &mut app, "校验并导出");
    assert!(app.directory.is_some() && app.export_confirmation.is_none());
    assert!(app.io_error.as_ref().unwrap().contains("绝对完整路径"));
    let inside = app.project.root.join("inside.zip");
    enter_path(&ctx, &mut app, inside.to_str().unwrap());
    click(&ctx, &mut app, "校验并导出");
    assert!(app.directory.is_some());
    assert!(app.io_error.as_ref().unwrap().contains("工作区外"));
    assert!(!inside.exists());
    let existing = zip_target(&app, "existing");
    fs::write(&existing, b"keep-existing").unwrap();
    enter_path(&ctx, &mut app, existing.to_str().unwrap());
    click(&ctx, &mut app, "校验并导出");
    assert!(app.directory.is_some());
    assert!(app.io_error.as_ref().unwrap().contains("已存在"));
    assert_eq!(fs::read(&existing).unwrap(), b"keep-existing");
    let original = app.project.document(&app.active_file).unwrap().to_owned();
    app.project
        .set_text(
            &app.active_file.clone(),
            "event broken\n  -> missing\n".into(),
        )
        .unwrap();
    let invalid = zip_target(&app, "invalid");
    enter_path(&ctx, &mut app, invalid.to_str().unwrap());
    click(&ctx, &mut app, "校验并导出");
    assert!(app.directory.is_some());
    assert!(app.io_error.as_ref().unwrap().contains("编译错误"));
    assert!(!invalid.exists());
    app.project
        .set_text(&app.active_file.clone(), original)
        .unwrap();
    click(&ctx, &mut app, "校验并导出");
    assert!(app.directory.is_none());
    assert!(invalid.exists());
    let _ = fs::remove_dir_all(app.project.root.parent().unwrap());
}

#[test]
fn zip_target_keeps_readonly_export_contract_and_reset_closes_both_decision_points() {
    let (ctx, mut app) = app();
    let manifest = app.project.root.join(".world/project.json");
    let bytes = app
        .project
        .authoring_document(&manifest)
        .unwrap()
        .bytes()
        .to_vec();
    // 未知能力来自磁盘旧/未来工程，正常打开只读；不能绕过受保护的编辑 API。
    fs::write(
        &manifest,
        String::from_utf8(bytes)
            .unwrap()
            .replace("content.entities.v1", "future.unknown.feature"),
    )
    .unwrap();
    app.project = worldline_core::project::Project::open(&app.project.root).unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    let expected = super::super::package::export_package_bytes(&app.project);
    app.export_package();
    let target = zip_target(&app, "readonly");
    enter_path(&ctx, &mut app, target.to_str().unwrap());
    click(&ctx, &mut app, "校验并导出");
    match expected {
        Ok(bytes) => assert_eq!(
            crate::archive::decode(&fs::read(&target).unwrap()).unwrap(),
            crate::archive::decode(&bytes).unwrap()
        ),
        Err(error) => {
            assert_eq!(app.io_error.as_deref(), Some(error.as_str()));
            assert!(app.directory.is_some() && !target.exists());
        }
    }
    assert_eq!(app.project.content_baseline(), baseline);
    app.export_package();
    app.reset_views();
    assert!(app.directory.is_none() && app.export_confirmation.is_none());
    let _ = fs::remove_dir_all(app.project.root.parent().unwrap());
}
