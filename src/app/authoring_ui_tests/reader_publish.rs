use super::*;

#[test]
fn reader_publish_entry_is_separate_and_explains_the_offline_boundary() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    let dirty = app.project.is_dirty();
    let destination = app
        .project
        .root
        .with_file_name(format!("reader-publish-empty-{}.zip", std::process::id()));
    let _ = std::fs::remove_file(&destination);
    click(&ctx, &mut app, 26, "发布给读者");
    replace_text_area(
        &ctx,
        &mut app,
        26,
        "reader-site.zip",
        &destination.to_string_lossy(),
    );
    assert!(
        rendered_text_in_window(&ctx, &mut app, 26, &destination.to_string_lossy())
            .contains(destination.to_string_lossy().as_ref()),
        "必须实际编辑目标路径后才能继续发布"
    );

    click(&ctx, &mut app, 26, "生成 / 更新预览");
    let output = frame(&ctx, &mut app, Vec::new(), 26);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("离线选择不是权限认证"), "{rendered}");
    assert!(
        rendered.contains("尚未生成预览；未选任何内容时不会创建空包。"),
        "{rendered}"
    );
    assert!(
        !destination.exists(),
        "empty selection must write no output"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.is_dirty(), dirty);
}

#[test]
fn reader_publish_uses_explicit_choices_and_publishes_the_reviewed_static_zip() {
    let (ctx, mut app) = reader_publish_app();
    let baseline = app.project.content_baseline();
    let dirty = app.project.is_dirty();
    let history_len = app.history.len();
    let destination = app
        .project
        .root
        .with_file_name(format!("reader-publish-ui-{}.zip", std::process::id()));
    let _ = std::fs::remove_file(&destination);

    click(&ctx, &mut app, 26, "发布给读者");
    click(&ctx, &mut app, 26, "Public Event · event (public)");
    click(&ctx, &mut app, 26, "Public Chapter (public-chapter)");
    click(&ctx, &mut app, 26, "Public Cover (cover)");
    replace_text_area(
        &ctx,
        &mut app,
        26,
        "reader-site.zip",
        &destination.to_string_lossy(),
    );
    assert!(
        rendered_text_in_window(&ctx, &mut app, 26, &destination.to_string_lossy())
            .contains(destination.to_string_lossy().as_ref()),
        "必须实际编辑目标路径后才能继续发布"
    );
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);

    let output = frame(&ctx, &mut app, Vec::new(), 26);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("Public Event"), "{rendered}");
    assert!(rendered.contains("Public Chapter"), "{rendered}");
    assert!(rendered.contains("Public Cover"), "{rendered}");
    let exclusions = "查看排除明细（11 项；仅供作者核对）";
    let rendered = scroll_from_visible_anchor_to(&ctx, &mut app, 26, "作者只读预览", exclusions);
    assert!(rendered.contains("未公开内容"), "{rendered}");
    scroll_from_visible_anchor_to(&ctx, &mut app, 26, exclusions, "发布 ZIP");
    assert!(!destination.exists(), "preview must not write output");
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.is_dirty(), dirty);
    assert_eq!(app.history.len(), history_len);

    click(&ctx, &mut app, 26, "发布 ZIP");
    assert!(
        !destination.exists(),
        "publish must require explicit confirmation"
    );
    click(
        &ctx,
        &mut app,
        26,
        "我已逐项核对预览，确认只发布以上离线内容（不代表在线权限控制）",
    );
    click(&ctx, &mut app, 26, "发布 ZIP");
    assert!(destination.is_file());
    let zip = std::fs::read(&destination).unwrap();
    let files = crate::archive::decode(&zip).unwrap();
    assert!(files.contains_key(std::path::Path::new("index.html")));
    assert!(files.contains_key(std::path::Path::new("search-index.json")));
    assert!(files.contains_key(std::path::Path::new("search.html")));
    assert!(files.contains_key(std::path::Path::new("reader.js")));
    assert!(files.contains_key(std::path::Path::new("style.css")));
    assert!(!files.contains_key(std::path::Path::new("world.wl")));
    assert!(
        files
            .keys()
            .any(|path| path == std::path::Path::new("assets/a0001.png")),
        "{:?}",
        files.keys().collect::<Vec<_>>()
    );
    assert_reader_package_resources_resolve(&files);
    let reader_script = std::str::from_utf8(&files[std::path::Path::new("reader.js")]).unwrap();
    assert!(reader_script.contains("textContent"));
    assert!(!reader_script.contains("eval("));
    assert!(!reader_script.contains("://"));
    let emitted = files
        .iter()
        .flat_map(|(path, bytes)| {
            path.to_string_lossy()
                .bytes()
                .chain(bytes.iter().copied())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for secret in [
        "HIDDEN_EVENT_TITLE_SENTINEL",
        "HIDDEN_EVENT_BODY_SENTINEL",
        "HIDDEN_LINK_LABEL_SENTINEL",
        "HIDDEN_CHAPTER_TITLE_SENTINEL",
        "HIDDEN_CHAPTER_BODY_SENTINEL",
        "HIDDEN_ASSET_TITLE_SENTINEL",
        "HIDDEN_ATTACHMENT_BYTES_SENTINEL",
        "private",
    ] {
        assert!(
            !emitted
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()),
            "reader package leaked {secret}"
        );
    }
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.is_dirty(), dirty);
    assert_eq!(app.history.len(), history_len);
    let rendered =
        scroll_from_visible_anchor_to(&ctx, &mut app, 26, "作者只读预览", "阅读包已写入");
    assert!(rendered.contains("阅读包已写入"), "{rendered}");
    let _ = std::fs::remove_file(destination);
}

#[test]
fn reader_publish_cancel_after_preview_leaves_no_output_and_preserves_author_state() {
    let (ctx, mut app) = reader_publish_app();
    let baseline = app.project.content_baseline();
    let dirty = app.project.is_dirty();
    let destination = app
        .project
        .root
        .with_file_name(format!("reader-publish-cancel-{}.zip", std::process::id()));
    let _ = std::fs::remove_file(&destination);

    click(&ctx, &mut app, 26, "发布给读者");
    click(&ctx, &mut app, 26, "Public Event · event (public)");
    replace_text_area(
        &ctx,
        &mut app,
        26,
        "reader-site.zip",
        &destination.to_string_lossy(),
    );
    assert!(
        rendered_text_in_window(&ctx, &mut app, 26, &destination.to_string_lossy())
            .contains(destination.to_string_lossy().as_ref()),
        "必须实际编辑目标路径后才能继续发布"
    );
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);
    assert!(
        !destination.exists(),
        "preview must not write the destination"
    );
    click(&ctx, &mut app, 26, "取消发布");
    assert!(
        !destination.exists(),
        "cancel must not write the destination"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.is_dirty(), dirty);
    assert!(app.history.is_empty());
}

#[test]
fn reader_publish_recompile_refreshes_candidates_and_invalidates_the_old_review() {
    let (ctx, mut app) = reader_publish_app();
    let destination = app
        .project
        .root
        .with_file_name(format!("reader-publish-stale-{}.zip", std::process::id()));
    let _ = std::fs::remove_file(&destination);

    click(&ctx, &mut app, 26, "发布给读者");
    click(&ctx, &mut app, 26, "Public Event · event (public)");
    replace_text_area(
        &ctx,
        &mut app,
        26,
        "reader-site.zip",
        &destination.to_string_lossy(),
    );
    assert!(
        rendered_text_in_window(&ctx, &mut app, 26, &destination.to_string_lossy())
            .contains(destination.to_string_lossy().as_ref()),
        "必须实际编辑目标路径后才能继续发布"
    );
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);
    assert!(!destination.exists());

    let entry = app.project.entry.clone();
    let mut changed = app.project.document(&entry).unwrap().to_owned();
    changed = changed.replace("Published story body.", "Updated story body.");
    app.project.set_text(&entry, changed).unwrap();
    app.recompile();
    let edited_baseline = app.project.content_baseline();
    let dirty = app.project.is_dirty();
    let output = frame(&ctx, &mut app, Vec::new(), 26);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(
        rendered.contains("尚未生成预览；未选任何内容时不会创建空包。"),
        "{rendered}"
    );
    assert!(
        rendered.contains("Public Event · event (public)"),
        "{rendered}"
    );
    assert!(!destination.exists());
    assert_eq!(app.project.content_baseline(), edited_baseline);
    assert_eq!(app.project.is_dirty(), dirty);
    assert!(app.history.is_empty());
    let _ = std::fs::remove_file(destination);
}

#[test]
fn reader_publish_cancel_and_existing_target_failure_never_write_or_overwrite() {
    let (ctx, mut app) = reader_publish_app();
    let baseline = app.project.content_baseline();
    let destination = app.project.root.with_file_name(format!(
        "reader-publish-existing-{}.zip",
        std::process::id()
    ));
    let sentinel = b"existing-output-must-survive";
    std::fs::write(&destination, sentinel).unwrap();

    click(&ctx, &mut app, 26, "发布给读者");
    click(&ctx, &mut app, 26, "Public Event · event (public)");
    replace_text_area(
        &ctx,
        &mut app,
        26,
        "reader-site.zip",
        &destination.to_string_lossy(),
    );
    assert!(
        rendered_text_in_window(&ctx, &mut app, 26, &destination.to_string_lossy())
            .contains(destination.to_string_lossy().as_ref()),
        "必须实际编辑目标路径后才能继续发布"
    );
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);
    click(
        &ctx,
        &mut app,
        26,
        "我已逐项核对预览，确认只发布以上离线内容（不代表在线权限控制）",
    );
    click(&ctx, &mut app, 26, "发布 ZIP");
    assert_eq!(std::fs::read(&destination).unwrap(), sentinel);
    let output = frame(&ctx, &mut app, Vec::new(), 26);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("发布失败"), "{rendered}");
    assert_eq!(app.project.content_baseline(), baseline);

    click(&ctx, &mut app, 26, "取消发布");
    let output = frame(&ctx, &mut app, Vec::new(), 26);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(!rendered.contains("离线选择不是权限认证"), "{rendered}");
    assert_eq!(std::fs::read(&destination).unwrap(), sentinel);
    assert_eq!(app.project.content_baseline(), baseline);
    let _ = std::fs::remove_file(destination);
}
