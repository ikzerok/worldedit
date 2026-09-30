use super::*;

#[test]
fn pinning_two_reading_panels_keeps_source_and_undo_unchanged() {
    let (ctx, mut app) = app();
    app.personal.settings.dock_references = false; // 此组专门验证保留的浮动模式
    let baseline = app.project.content_baseline();
    app.open_reading(TargetRef::new("entity", "a"));
    click(&ctx, &mut app, 8, "钉住旁查");
    app.open_reading(TargetRef::new("entity", "b"));
    click(&ctx, &mut app, 8, "钉住旁查");
    assert_eq!(app.reading_panels.ids().len(), 2);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    app.reset_views();
    assert!(app.reading_panels.ids().is_empty());
}

#[test]
fn two_pinned_panels_keep_targets_and_back_history_independent_through_real_links() {
    let (ctx, mut app) = app();
    app.personal.settings.dock_references = false; // 此组专门验证保留的浮动模式
    let entry = app.project.entry.clone();
    let mut source = app.project.sources()[&entry].clone();
    source.push_str(concat!(
        "entity c kind place as \"第三资料\"\n",
        "relation_def r1 type knows from entity a to entity c\n",
        "relation_def r2 type knows from entity b to entity c\n",
    ));
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    app.project.mark_saved();
    let baseline = app.project.content_baseline();

    app.open_reading(TargetRef::new("relation", "r1"));
    click(&ctx, &mut app, 8, "钉住旁查");
    app.open_reading(TargetRef::new("relation", "r2"));
    click(&ctx, &mut app, 8, "钉住旁查");
    let ids = app.reading_panels.ids();
    let first = ids[0];
    let second = ids[1];

    let output = frame(&ctx, &mut app, Vec::new(), 8);
    let mut wide = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut wide);
    }
    assert!(wide.contains("旁查 1 · relation:r1"), "{wide}");
    assert!(wide.contains("旁查 2 · relation:r2"), "{wide}");
    click(&ctx, &mut app, 8, "实体 · 同名 · a");
    assert_eq!(
        app.reading_panels.get(first).unwrap().target,
        TargetRef::new("entity", "a")
    );
    assert_eq!(
        app.reading_panels.get(second).unwrap().target,
        TargetRef::new("relation", "r2")
    );

    click(&ctx, &mut app, 9, "实体 · 同名 · b");
    assert_eq!(
        app.reading_panels.get(second).unwrap().target,
        TargetRef::new("entity", "b")
    );
    assert_eq!(
        app.reading_panels.get(first).unwrap().target,
        TargetRef::new("entity", "a")
    );
    let output = frame(&ctx, &mut app, Vec::new(), 9);
    let mut narrow = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut narrow);
    }
    assert!(narrow.contains("旁查 2 · entity:b"), "{narrow}");
    assert!(!narrow.contains("旁查 1 · entity:a"), "{narrow}");

    app.selected_reading_panel = Some(first);
    click(&ctx, &mut app, 9, "← 返回");
    assert_eq!(
        app.reading_panels.get(first).unwrap().target,
        TargetRef::new("relation", "r1")
    );
    assert_eq!(
        app.reading_panels.get(second).unwrap().target,
        TargetRef::new("entity", "b")
    );
    app.selected_reading_panel = Some(second);
    click(&ctx, &mut app, 9, "← 返回");
    assert_eq!(
        app.reading_panels.get(second).unwrap().target,
        TargetRef::new("relation", "r2")
    );
    assert_eq!(
        app.reading_panels.get(first).unwrap().target,
        TargetRef::new("relation", "r1")
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(!app.project.is_dirty());
    assert!(app.history.is_empty());
}

#[test]
fn narrow_reading_panels_can_switch_and_close_without_losing_an_edit_draft() {
    let (ctx, mut app) = app();
    app.personal.settings.dock_references = false; // 此组专门验证保留的浮动模式
    let first = app
        .reading_panels
        .pin(TargetRef::new("entity", "a"))
        .unwrap();
    let second = app
        .reading_panels
        .pin(TargetRef::new("entity", "b"))
        .unwrap();
    app.selected_reading_panel = Some(first);
    app.edit_entity(Some("a"));
    app.entity_editor.as_mut().unwrap().draft.description = "未提交资料".into();
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 9, "旁查 2");
    assert_eq!(app.selected_reading_panel, Some(second));
    click(&ctx, &mut app, 9, "返回源码编辑");
    assert_eq!(app.tab, super::Tab::Edit);
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        Some(egui::Id::new(("source", &app.active_file)))
    );
    click(&ctx, &mut app, 9, "编辑此对象");
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.description,
        "未提交资料"
    );
    click(&ctx, &mut app, 9, "关闭旁查");
    assert!(app.reading_panels.get(second).is_none());
    assert!(app.reading_panels.get(first).is_some());
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.description,
        "未提交资料"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn project_switch_preserves_unsubmitted_form_even_with_clean_project() {
    let (ctx, mut app) = app();
    app.personal.settings.dock_references = false; // 此组专门验证保留的浮动模式
    app.project.mark_saved();
    app.edit_entity(Some("a"));
    app.entity_editor.as_mut().unwrap().draft.description = "未提交资料".into();
    app.request_action(super::Pending::Close, &ctx);
    assert!(!app.allow_close);
    assert!(app.pending.is_none());
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.description,
        "未提交资料"
    );
}

#[test]
fn opening_another_project_clears_all_personal_reading_targets() {
    let (ctx, mut app) = app();
    app.personal.settings.dock_references = false; // 此组专门验证保留的浮动模式
    let original_root = app.project.root.clone();
    app.project.mark_saved();
    app.open_reading(TargetRef::new("entity", "a"));
    let first = app
        .reading_panels
        .pin(TargetRef::new("entity", "a"))
        .unwrap();
    let second = app
        .reading_panels
        .pin(TargetRef::new("entity", "b"))
        .unwrap();
    app.reading_panels
        .navigate(first, TargetRef::new("entity", "b"));
    app.selected_reading_panel = Some(second);

    let next_root = std::env::temp_dir().join(format!(
        "worldedit-reading-project-switch-{}-{}",
        std::process::id(),
        NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut next_project = Project::new(&next_root);
    let next_entry = next_project.entry.clone();
    next_project
        .set_text(&next_entry, "event fresh as \"新工程\"\n  -> END\n".into())
        .unwrap();
    next_project.save().unwrap();

    app.request_action(super::Pending::Open(next_root), &ctx);
    assert_ne!(app.project.root, original_root);
    assert!(app.reading_target.is_none());
    assert!(app.reading_history.is_empty());
    assert!(app.reading_panels.ids().is_empty());
    assert!(app.active_reading_panel.is_none());
    assert!(app.selected_reading_panel.is_none());
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .object(&TargetRef::new("event", "fresh"))
        .is_some());
}

#[test]
fn opening_another_project_is_blocked_without_dropping_pinned_context_or_form_draft() {
    let (ctx, mut app) = app();
    app.personal.settings.dock_references = false; // 此组专门验证保留的浮动模式
    let original_root = app.project.root.clone();
    app.project.mark_saved();
    let panel = app
        .reading_panels
        .pin(TargetRef::new("entity", "a"))
        .unwrap();
    app.edit_entity(Some("a"));
    app.entity_editor.as_mut().unwrap().draft.description = "未提交资料旁注".into();

    let next_root = std::env::temp_dir().join(format!(
        "worldedit-reading-project-blocked-{}-{}",
        std::process::id(),
        NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut next_project = Project::new(&next_root);
    next_project.save().unwrap();

    app.request_action(super::Pending::Open(next_root), &ctx);
    assert_eq!(app.project.root, original_root);
    assert_eq!(app.reading_panels.get(panel).unwrap().target.id, "a");
    assert!(app.pending.is_none());
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.description,
        "未提交资料旁注"
    );
    assert!(app
        .message
        .as_deref()
        .is_some_and(|message| message.contains("输入已保留")));
}

#[test]
fn project_switch_preserves_an_unsubmitted_period_form() {
    let (ctx, mut app) = app();
    app.personal.settings.dock_references = false; // 此组专门验证保留的浮动模式
    app.project.mark_saved();
    app.new_period = Some(("age".into(), "未提交时代".into(), None));
    app.request_action(super::Pending::Close, &ctx);
    assert!(!app.allow_close);
    assert!(app.pending.is_none());
    assert_eq!(app.new_period.as_ref().unwrap().1, "未提交时代");
}

#[test]
fn pinned_wiki_navigation_does_not_close_an_independent_temporary_reader() {
    let (ctx, mut app) = app();
    app.personal.settings.dock_references = false; // 此组专门验证保留的浮动模式
    let id = app
        .reading_panels
        .pin(TargetRef::new("entity", "a"))
        .unwrap();
    app.open_reading(TargetRef::new("entity", "b"));
    app.active_reading_panel = Some(id);
    click(&ctx, &mut app, 10, "在 Wiki 中查看");
    assert_eq!(app.reading_target, Some(TargetRef::new("entity", "b")));
    assert_eq!(app.wiki_target, Some(TargetRef::new("entity", "a")));
    let hit = &app
        .snapshot
        .as_ref()
        .unwrap()
        .wiki
        .occurrences(&TargetRef::new("entity", "a"))[0];
    let source_button = format!(
        "{}:{}:{}",
        hit.file.strip_prefix(&app.project.root).unwrap().display(),
        hit.line,
        hit.column
    );
    click(&ctx, &mut app, 10, &source_button);
    assert_eq!(app.reading_target, Some(TargetRef::new("entity", "b")));
}

#[test]
fn pinned_target_refreshes_updates_and_never_falls_back_to_a_same_name_id() {
    let (ctx, mut app) = app();
    app.personal.settings.dock_references = false; // 此组专门验证保留的浮动模式
    let id = app
        .reading_panels
        .pin(TargetRef::new("entity", "a"))
        .unwrap();
    let entry = app.project.entry.clone();
    app.project
        .set_text(
            &entry,
            "entity a kind place as \"更新后同名\"\nentity b kind organization as \"更新后同名\"\n"
                .into(),
        )
        .unwrap();
    app.recompile();
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 9);
    }
    let updated = frame(&ctx, &mut app, Vec::new(), 9);
    let mut rendered = String::new();
    for shape in &updated.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("更新后同名"), "{rendered}");

    app.project
        .set_text(&entry, "entity b kind place as \"更新后同名\"\n".into())
        .unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    let dirty = app.project.is_dirty();
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 9);
    }
    let output = frame(&ctx, &mut app, Vec::new(), 9);
    assert!(output.shapes.iter().any(|shape| text_position(
        &shape.shape,
        "资料已失效：entity:a。可能已被删除或更改 ID。"
    )
    .is_some()));
    assert_eq!(app.reading_panels.get(id).unwrap().target.id, "a");
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.is_dirty(), dirty);
    assert!(app.history.is_empty());
}
