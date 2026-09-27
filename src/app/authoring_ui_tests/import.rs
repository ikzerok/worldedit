use super::*;

fn markdown_import_fixture(markdown: &[u8]) -> (std::path::PathBuf, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "worldedit-markdown-import-ui-{}-{}",
        std::process::id(),
        NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    let source = root.join("source");
    let target = root.join("empty-target");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(source.join("page.md"), markdown).unwrap();
    (source, target)
}

fn open_markdown_import(ctx: &egui::Context, app: &mut WorldeditApp) {
    click(ctx, app, 25, "工程");
    click(ctx, app, 25, "导入 Markdown…");
}

#[test]
fn markdown_import_preview_and_cancel_keep_the_target_empty_and_show_losses() {
    let (ctx, mut app) = app();
    let (source, target) = markdown_import_fixture(
        b"---\ntitle: Imported Page\ncustom_field: keep\n---\n\nA **formatted** paragraph.\n",
    );
    let baseline = app.project.content_baseline();
    open_markdown_import(&ctx, &mut app);
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择 Markdown 来源目录…",
        &source.display().to_string(),
    );
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择空工程目录…",
        &target.display().to_string(),
    );
    click(&ctx, &mut app, 17, "预检导入");
    scroll_rendered_text(&ctx, &mut app, 17, "损失预览 ·");
    click_containing(&ctx, &mut app, 17, "损失预览 ·");
    let preview = format!(
        "{}{}",
        scroll_rendered_text(&ctx, &mut app, 17, "UNSUPPORTED_FRONT_MATTER_FIELD"),
        scroll_rendered_text(&ctx, &mut app, 17, "UNSUPPORTED_INLINE_MARKUP")
    );

    assert!(
        preview.contains("UNSUPPORTED_FRONT_MATTER_FIELD"),
        "{preview}"
    );
    assert!(preview.contains("UNSUPPORTED_INLINE_MARKUP"), "{preview}");
    assert!(preview.contains("page.md"), "{preview}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(!target.join(".world").exists());
    assert_eq!(
        std::fs::read(source.join("page.md")).unwrap(),
        b"---\ntitle: Imported Page\ncustom_field: keep\n---\n\nA **formatted** paragraph.\n"
    );

    click(&ctx, &mut app, 17, "取消");
    assert!(!target.join(".world").exists());
    assert!(std::fs::read_dir(&target).unwrap().next().is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    std::fs::remove_dir_all(source.parent().unwrap()).unwrap();
}

#[test]
fn markdown_import_files_snapshot_preflight_uses_core_and_cancel_keeps_targets_unchanged() {
    let (ctx, mut app) = app();
    let markdown =
        b"---\ntitle: Snapshot Page\ncustom_field: keep\n---\n\nA **formatted** paragraph.\n";
    let (source, target) = markdown_import_fixture(markdown);
    open_markdown_import(&ctx, &mut app);
    app.markdown_import_wizard
        .as_mut()
        .unwrap()
        .set_source_files(worldline_core::workspace_snapshot::Files::from([(
            std::path::PathBuf::from("page.md"),
            markdown.to_vec(),
        )]));
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择空工程目录…",
        &target.display().to_string(),
    );
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 17, "预检导入");

    let preview = rendered_text_in_window(&ctx, &mut app, 17, "预检完成：");
    assert!(preview.contains("1 个页面"), "{preview}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(std::fs::read_dir(&target).unwrap().next().is_none());

    click(&ctx, &mut app, 17, "取消");
    assert!(std::fs::read_dir(&target).unwrap().next().is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    std::fs::remove_dir_all(source.parent().unwrap()).unwrap();
}

#[test]
fn markdown_import_apply_requires_loss_confirmation_and_round_trips_original_markdown() {
    let (ctx, mut app) = app();
    let original =
        b"---\ntitle: Imported Page\ncustom_field: keep\n---\n\nA **formatted** paragraph.\n";
    let (source, target) = markdown_import_fixture(original);
    open_markdown_import(&ctx, &mut app);
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择 Markdown 来源目录…",
        &source.display().to_string(),
    );
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择空工程目录…",
        &target.display().to_string(),
    );
    click(&ctx, &mut app, 17, "预检导入");
    scroll_rendered_text(&ctx, &mut app, 17, "损失预览 ·");
    click_containing(&ctx, &mut app, 17, "损失预览 ·");
    click(&ctx, &mut app, 17, "应用导入");
    assert!(std::fs::read_dir(&target).unwrap().next().is_none());
    assert!(
        rendered_text_in_window(&ctx, &mut app, 17, "我已检查并接受预览中的损失")
            .contains("我已检查并接受预览中的损失")
    );

    click(&ctx, &mut app, 17, "我已检查并接受预览中的损失");
    click(&ctx, &mut app, 17, "我已检查目标语言版本升级及其影响");
    click(&ctx, &mut app, 17, "应用导入");

    assert_eq!(app.project.root, Project::open(&target).unwrap().root);
    assert!(app.saved_location);
    assert!(app.history.is_empty());
    assert!(app
        .message
        .as_deref()
        .is_some_and(|message| message.contains("1 个页面")));
    let mut reopened = Project::open(&target).unwrap();
    assert!(!reopened.compile().has_errors());
    assert!(reopened
        .export_files()
        .unwrap()
        .values()
        .any(|bytes| bytes.as_slice() == original));
    std::fs::remove_dir_all(source.parent().unwrap()).unwrap();
}

#[test]
fn markdown_import_rejects_a_source_that_changes_after_preview_without_writing() {
    let (ctx, mut app) = app();
    let (source, target) = markdown_import_fixture(
        b"---\ntitle: Stable Page\n---\n\nPlain text without conversion losses.\n",
    );
    open_markdown_import(&ctx, &mut app);
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择 Markdown 来源目录…",
        &source.display().to_string(),
    );
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择空工程目录…",
        &target.display().to_string(),
    );
    click(&ctx, &mut app, 17, "预检导入");
    assert!(rendered_text_in_window(&ctx, &mut app, 17, "应用导入").contains("应用导入"));

    let changed_source =
        b"---\ntitle: Changed Page\n---\n\nPlain text without conversion losses.\n";
    std::fs::write(source.join("page.md"), changed_source).unwrap();
    click(&ctx, &mut app, 17, "我已检查目标语言版本升级及其影响");
    let before_apply = rendered_text_in_window(&ctx, &mut app, 17, "应用导入");
    assert!(!before_apply.contains("尚未确认语言升级"), "{before_apply}");
    if before_apply.contains("我已检查并接受预览中的损失") {
        click(&ctx, &mut app, 17, "我已检查并接受预览中的损失");
    }
    let before_stale_apply = rendered_text_in_window(&ctx, &mut app, 17, "应用导入");
    assert!(
        !before_stale_apply.contains("尚未确认损失"),
        "{before_stale_apply}"
    );
    click(&ctx, &mut app, 17, "应用导入");
    let rendered = rendered_text_in_window(&ctx, &mut app, 17, "已过期");

    assert!(rendered.contains("已过期"), "{rendered}");
    assert!(rendered.contains("重新预检"), "{rendered}");
    assert!(!target.join(".world").exists());
    assert!(std::fs::read_dir(&target).unwrap().next().is_none());
    click(&ctx, &mut app, 17, "重新预检");
    let ready = rendered_text_in_window(&ctx, &mut app, 17, "预检完成：");
    assert!(ready.contains("0 个阻塞冲突"), "{ready}");
    let ready_to_apply = rendered_text_in_window(&ctx, &mut app, 17, "必需确认已完成");
    assert!(
        ready_to_apply.contains("必需确认已完成"),
        "{ready_to_apply}"
    );
    click_containing(&ctx, &mut app, 17, "页面映射 ·");
    let refreshed = scroll_rendered_text(&ctx, &mut app, 17, "Changed Page");
    assert!(refreshed.contains("Changed Page"), "{refreshed}");
    scroll_window_to_top(&ctx, &mut app, 17);
    click(&ctx, &mut app, 17, "应用导入");
    assert_eq!(
        app.project.root,
        Project::open(&target).unwrap().root,
        "message={:?}, io_error={:?}",
        app.message,
        app.io_error
    );
    let mut reopened = Project::open(&target).unwrap();
    let result = reopened.compile();
    assert!(!result.has_errors());
    assert!(reopened
        .export_files()
        .unwrap()
        .values()
        .any(|bytes| bytes.as_slice() == changed_source));
    std::fs::remove_dir_all(source.parent().unwrap()).unwrap();
}

#[test]
fn markdown_import_resolves_a_real_id_conflict_without_merging_same_name_targets() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    app.saved_location = true;
    let root = app.project.root.clone();
    let (source, _) = markdown_import_fixture(
        "---\nid: a\ntitle: \"同名\"\n---\n\nA separately imported page.\n".as_bytes(),
    );
    open_markdown_import(&ctx, &mut app);
    click(&ctx, &mut app, 17, "导入到当前工程");
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择 Markdown 来源目录…",
        &source.display().to_string(),
    );
    click(&ctx, &mut app, 17, "预检导入");
    scroll_rendered_text(&ctx, &mut app, 17, "阻塞冲突 ·");
    click_containing(&ctx, &mut app, 17, "阻塞冲突 ·");
    let conflict = format!(
        "{}{}",
        scroll_rendered_text(&ctx, &mut app, 17, "ENTITY_ID_CONFLICT"),
        scroll_rendered_text(&ctx, &mut app, 17, "a_import_2")
    );
    assert!(conflict.contains("page.md"), "{conflict}");
    assert!(conflict.contains("a_import_2"), "{conflict}");
    click_containing(&ctx, &mut app, 17, "阻塞冲突 ·");
    scroll_window_to_top(&ctx, &mut app, 17);
    scroll_rendered_text(&ctx, &mut app, 17, "同名资料提示 ·");
    click_containing(&ctx, &mut app, 17, "同名资料提示 ·");
    let name_conflict = scroll_rendered_text(&ctx, &mut app, 17, "工程中存在同名资料");
    assert!(name_conflict.contains("entity:a"), "{name_conflict}");
    assert!(name_conflict.contains("entity:b"), "{name_conflict}");

    scroll_window_to_top(&ctx, &mut app, 17);
    click_containing(&ctx, &mut app, 17, "同名资料提示 ·");
    click_containing(&ctx, &mut app, 17, "阻塞冲突 ·");
    scroll_rendered_text(&ctx, &mut app, 17, "a_import_2");
    click(&ctx, &mut app, 17, "a_import_2");
    click(&ctx, &mut app, 17, "预检导入");
    let ready = rendered_text_in_window(&ctx, &mut app, 17, "预检完成：");
    assert!(ready.contains("0 个阻塞冲突"), "{ready}");
    if ready.contains("我已检查并接受预览中的损失") {
        click(&ctx, &mut app, 17, "我已检查并接受预览中的损失");
    }
    if ready.contains("我已检查目标语言版本升级及其影响") {
        click(&ctx, &mut app, 17, "我已检查目标语言版本升级及其影响");
    }
    click_containing(&ctx, &mut app, 17, "页面映射 ·");
    let resolved = rendered_text_in_window(&ctx, &mut app, 17, "entity:a_import_2");
    assert!(resolved.contains("page.md"), "{resolved}");
    click(&ctx, &mut app, 17, "应用导入");

    assert!(!app.project.is_dirty());
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert!(app
        .message
        .as_deref()
        .is_some_and(|message| message.contains("导入已保存")));
    let mut reopened = Project::open(&root).unwrap();
    let result = reopened.compile();
    assert!(!result.has_errors());
    assert!(result.analysis.catalog.entities.contains_key("a_import_2"));
    assert!(result.analysis.catalog.entities.contains_key("a"));
    std::fs::remove_dir_all(source.parent().unwrap()).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
