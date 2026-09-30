use super::*;

#[test]
fn checkpoint_history_creates_lists_previews_cancels_restores_and_undoes_through_real_ui() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let source = app.active_file.clone();
    let checkpoint_source = app.project.document(&source).unwrap().to_owned();
    let baseline = app.project.content_baseline();
    let history_len = app.history.len();

    app.tab = super::Tab::CheckpointHistory;
    app.checkpoint_history.label = "恢复测试点".into();
    click(&ctx, &mut app, 23, "创建检查点");

    let checkpoint = app
        .project
        .list_checkpoints()
        .unwrap()
        .into_iter()
        .find(|record| record.label.as_deref() == Some("恢复测试点"))
        .unwrap();
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.history.len(), history_len);
    assert_eq!(app.project.document(&source).unwrap(), checkpoint_source);
    assert_eq!(app.project.list_checkpoints().unwrap().len(), 1);

    let opened = Project::open(&source).unwrap();
    assert!(opened
        .list_checkpoints()
        .unwrap()
        .iter()
        .any(|record| record.id == checkpoint.id));

    let changed = format!("{checkpoint_source}\n// 检查点之后的本地草稿\n");
    app.project.set_text(&source, changed.clone()).unwrap();
    app.recompile();
    click(&ctx, &mut app, 23, "预览恢复…");
    let preview = frame(&ctx, &mut app, Vec::new(), 23);
    let mut preview_text = String::new();
    for shape in &preview.shapes {
        collect_text(&shape.shape, &mut preview_text);
    }
    assert!(preview_text.contains("恢复预览"), "{preview_text}");
    assert!(preview_text.contains("逐文件操作"), "{preview_text}");
    assert!(preview_text.contains("三方文本差异"), "{preview_text}");
    assert!(preview_text.contains("world.wl"), "{preview_text}");

    click(&ctx, &mut app, 23, "文本差异：world.wl");
    let expanded = frame(&ctx, &mut app, Vec::new(), 23);
    let mut expanded_text = String::new();
    for shape in &expanded.shapes {
        collect_text(&shape.shape, &mut expanded_text);
    }
    assert!(expanded_text.contains("base"), "{expanded_text}");
    assert!(expanded_text.contains("current"), "{expanded_text}");
    assert!(expanded_text.contains("checkpoint"), "{expanded_text}");
    assert!(
        expanded_text.contains("检查点之后的本地草稿"),
        "{expanded_text}"
    );

    click(&ctx, &mut app, 23, "打开恢复确认…");
    click(&ctx, &mut app, 23, "取消恢复");
    assert_eq!(app.project.document(&source).unwrap(), changed);
    assert_eq!(app.project.list_checkpoints().unwrap().len(), 1);

    click(&ctx, &mut app, 23, "预览恢复…");
    click(&ctx, &mut app, 23, "打开恢复确认…");
    assert!(app.checkpoint_history.restore_confirmation);
    click(&ctx, &mut app, 23, "确认恢复此工程检查点");
    assert_eq!(app.project.document(&source).unwrap(), checkpoint_source);
    assert!(!app.project.is_dirty());
    assert_eq!(app.history.len(), history_len + 1);

    app.undo(false);
    assert_eq!(app.project.document(&source).unwrap(), changed);
    assert!(app.project.is_dirty());
}

#[test]
fn stale_checkpoint_confirmation_is_disabled_until_a_new_preview_is_opened() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let source = app.active_file.clone();
    let checkpoint_source = app.project.document(&source).unwrap().to_owned();
    let checkpoint = app
        .project
        .create_checkpoint(
            Some("陈旧预览".into()),
            worldline_core::project::CheckpointLimits::default(),
        )
        .unwrap();

    app.tab = super::Tab::CheckpointHistory;
    app.checkpoint_history.selected_id = Some(checkpoint.id);
    let after_preview = format!("{checkpoint_source}\n// 预览时的草稿\n");
    app.project
        .set_text(&source, after_preview.clone())
        .unwrap();
    app.recompile();
    click(&ctx, &mut app, 23, "预览恢复…");
    click(&ctx, &mut app, 23, "打开恢复确认…");

    let newer_draft = format!("{after_preview}\n// 之后的草稿\n");
    app.project.set_text(&source, newer_draft.clone()).unwrap();
    app.recompile();
    let output = frame(&ctx, &mut app, Vec::new(), 23);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("此预览已过期"), "{rendered}");
    assert_eq!(app.project.document(&source).unwrap(), newer_draft);

    click(&ctx, &mut app, 23, "取消恢复");
    assert_eq!(app.project.document(&source).unwrap(), newer_draft);
    let records = app.project.list_checkpoints().unwrap();
    assert_eq!(records.len(), 1);
}

#[test]
fn escape_cancels_checkpoint_confirmation_and_small_window_keeps_history_controls_visible() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let source = app.active_file.clone();
    let original = app.project.document(&source).unwrap().to_owned();
    let checkpoint = app
        .project
        .create_checkpoint(
            Some("键盘取消".into()),
            worldline_core::project::CheckpointLimits::default(),
        )
        .unwrap();
    app.tab = super::Tab::CheckpointHistory;
    app.checkpoint_history.selected_id = Some(checkpoint.id);

    let small = frame(&ctx, &mut app, Vec::new(), 24);
    let mut rendered = String::new();
    for shape in &small.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("检查点历史"), "{rendered}");
    assert!(rendered.contains("创建检查点"), "{rendered}");
    assert!(rendered.contains("所选记录"), "{rendered}");

    let draft = format!("{original}\n// Escape 应保留的草稿\n");
    app.project.set_text(&source, draft.clone()).unwrap();
    app.recompile();
    click(&ctx, &mut app, 23, "预览恢复…");
    click(&ctx, &mut app, 23, "打开恢复确认…");
    assert!(app.checkpoint_history.restore_confirmation);
    let escape = Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = frame(&ctx, &mut app, vec![escape], 23);
    assert!(!app.checkpoint_history.restore_confirmation);
    assert!(app.checkpoint_history.preview.is_none());
    assert_eq!(app.project.document(&source).unwrap(), draft);
}

#[test]
fn checkpoint_quota_failure_keeps_label_and_project_and_delete_only_removes_history() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let source = app.active_file.clone();
    let contents = app.project.document(&source).unwrap().to_owned();
    let baseline = app.project.content_baseline();
    for _ in 0..20 {
        app.project
            .create_checkpoint(None, worldline_core::project::CheckpointLimits::default())
            .unwrap();
    }
    let history_len = app.history.len();
    app.tab = super::Tab::CheckpointHistory;
    app.checkpoint_history.label = "第 21 条".into();
    click(&ctx, &mut app, 23, "创建检查点");
    assert!(app
        .checkpoint_history
        .error
        .as_deref()
        .is_some_and(|error| error.contains("配额")));
    assert_eq!(app.checkpoint_history.label, "第 21 条");
    assert_eq!(app.project.list_checkpoints().unwrap().len(), 20);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&source).unwrap(), contents);
    assert_eq!(app.history.len(), history_len);

    let record = app.project.list_checkpoints().unwrap().remove(0);
    app.checkpoint_history.selected_id = Some(record.id.clone());
    click(&ctx, &mut app, 23, "删除历史记录…");
    let confirmation = frame(&ctx, &mut app, Vec::new(), 23);
    let mut confirmation_text = String::new();
    for shape in &confirmation.shapes {
        collect_text(&shape.shape, &mut confirmation_text);
    }
    assert!(confirmation_text.contains("不会删除或更改当前工程文件"));
    click(&ctx, &mut app, 23, "确认删除历史记录");
    assert_eq!(app.project.list_checkpoints().unwrap().len(), 19);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&source).unwrap(), contents);
    assert_eq!(app.history.len(), history_len);
}

#[test]
fn checkpoint_history_filter_and_unavailable_record_stay_read_only_and_non_executable() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let source = app.active_file.clone();
    let contents = app.project.document(&source).unwrap().to_owned();
    let baseline = app.project.content_baseline();
    let alpha = app
        .project
        .create_checkpoint(
            Some("alpha review".into()),
            worldline_core::project::CheckpointLimits::default(),
        )
        .unwrap();
    app.project
        .create_checkpoint(
            Some("beta review".into()),
            worldline_core::project::CheckpointLimits::default(),
        )
        .unwrap();
    app.tab = super::Tab::CheckpointHistory;
    app.checkpoint_history.query = "alpha".into();
    let filtered = frame(&ctx, &mut app, Vec::new(), 23);
    let mut filtered_text = String::new();
    for shape in &filtered.shapes {
        collect_text(&shape.shape, &mut filtered_text);
    }
    assert!(filtered_text.contains("alpha review"), "{filtered_text}");
    assert!(!filtered_text.contains("beta review"), "{filtered_text}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&source).unwrap(), contents);
    assert_eq!(app.project.list_checkpoints().unwrap().len(), 2);

    let payload_directory = app
        .project
        .root
        .join(".world/.checkpoints/v1")
        .join(&alpha.id)
        .join("files");
    let payload = std::fs::read_dir(payload_directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(payload, b"damaged checkpoint payload").unwrap();
    let damaged = app
        .project
        .list_checkpoints()
        .unwrap()
        .into_iter()
        .find(|record| record.id == alpha.id)
        .unwrap();
    assert!(!damaged.available);
    app.checkpoint_history.query.clear();
    app.checkpoint_history.selected_id = Some(alpha.id);
    let output = frame(&ctx, &mut app, Vec::new(), 23);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("此记录不可恢复"), "{rendered}");
    click(&ctx, &mut app, 23, "预览恢复…");
    assert!(app.checkpoint_history.preview.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&source).unwrap(), contents);
}

#[test]
fn checkpoint_confirmation_repreviews_external_workspace_changes_before_restoring() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let source = app.active_file.clone();
    let original = app.project.document(&source).unwrap().to_owned();
    let checkpoint = app
        .project
        .create_checkpoint(
            Some("磁盘变化前".into()),
            worldline_core::project::CheckpointLimits::default(),
        )
        .unwrap();
    let draft = format!("{original}\n// 保留这个工程草稿\n");
    app.project.set_text(&source, draft.clone()).unwrap();
    app.recompile();
    app.tab = super::Tab::CheckpointHistory;
    app.checkpoint_history.selected_id = Some(checkpoint.id);
    click(&ctx, &mut app, 23, "预览恢复…");
    click(&ctx, &mut app, 23, "打开恢复确认…");

    let external_file = app.project.root.join("external-note.txt");
    std::fs::write(&external_file, "外部新文件").unwrap();
    let _ = frame(&ctx, &mut app, Vec::new(), 23);
    click(&ctx, &mut app, 23, "确认恢复此工程检查点");

    assert_eq!(app.project.document(&source).unwrap(), draft);
    assert_eq!(
        std::fs::read_to_string(external_file).unwrap(),
        "外部新文件"
    );
    assert!(app.project.is_dirty());
    assert!(app.checkpoint_history.preview.is_none());
    assert!(app
        .checkpoint_history
        .error
        .as_deref()
        .is_some_and(|error| error.contains("过期")));
    assert!(app.project.list_checkpoints().unwrap()[0].available);
}

fn scroll_checkpoint_details_to(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for delta in [-90.0, 90.0] {
        for _ in 0..100 {
            let output = frame(ctx, app, Vec::new(), 36);
            if visible_text_position(&output, label).is_some() {
                return;
            }
            let point = pos2(1000.0, 550.0);
            let _ = frame(
                ctx,
                app,
                vec![
                    Event::PointerMoved(point),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, delta),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                36,
            );
        }
    }
    panic!("宽屏检查点详情滚动后仍不可达：{label}");
}

#[test]
fn wide_checkpoint_long_details_can_scroll_to_cancel_and_confirm_restore() {
    let (ctx, mut app) = app();
    let source = app.active_file.clone();
    let original = (0..80).map(|index| {
        format!("entity checkpoint_related_object_{index:03} kind place as \"关联地点 {index:03}\"\n")
    }).collect::<String>();
    app.project.set_text(&source, original.clone()).unwrap();
    app.project.save().unwrap();
    let checkpoint = app
        .project
        .create_checkpoint(
            Some("长关联对象检查点".into()),
            worldline_core::project::CheckpointLimits::default(),
        )
        .unwrap();
    app.tab = super::Tab::CheckpointHistory;
    app.checkpoint_history.selected_id = Some(checkpoint.id.clone());
    let draft = format!("{original}\n// 需要保留的未保存草稿\n");
    app.project.set_text(&source, draft.clone()).unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 36, "预览恢复…");
    assert!(app
        .checkpoint_history
        .preview
        .as_ref()
        .unwrap()
        .changes
        .iter()
        .any(|change| change.affected_objects.len() >= 80));
    scroll_checkpoint_details_to(&ctx, &mut app, "打开恢复确认…");
    click(&ctx, &mut app, 36, "打开恢复确认…");
    scroll_checkpoint_details_to(&ctx, &mut app, "取消恢复");
    click(&ctx, &mut app, 36, "取消恢复");
    assert!(!app.checkpoint_history.restore_confirmation);
    assert!(app.checkpoint_history.preview.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&source).unwrap(), draft);
    assert_eq!(std::fs::read_to_string(&source).unwrap(), original);
    assert!(app.history.is_empty());
    assert_eq!(app.project.list_checkpoints().unwrap().len(), 1);

    click(&ctx, &mut app, 36, "预览恢复…");
    scroll_checkpoint_details_to(&ctx, &mut app, "打开恢复确认…");
    click(&ctx, &mut app, 36, "打开恢复确认…");
    assert!(app.checkpoint_history.restore_confirmation);
    scroll_checkpoint_details_to(&ctx, &mut app, "确认恢复此工程检查点");
    click(&ctx, &mut app, 36, "确认恢复此工程检查点");
    assert_eq!(app.project.document(&source).unwrap(), original);
    assert_eq!(std::fs::read_to_string(&source).unwrap(), original);
    assert!(!app.project.is_dirty());
    assert_eq!(app.history.len(), 1);
    app.undo(false);
    assert_eq!(app.project.document(&source).unwrap(), draft);
    assert!(app.project.is_dirty());
}
