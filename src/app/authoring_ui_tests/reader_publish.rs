use super::*;
mod narrow;

#[test]
fn reader_publish_entry_is_separate_and_explains_the_offline_boundary() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    let dirty = app.project.is_dirty();
    let destination = app.project.root.with_file_name(format!(
        "{}-reader-site.zip",
        app.project.root.file_name().unwrap().to_string_lossy()
    ));
    let _ = std::fs::remove_file(&destination);
    click(&ctx, &mut app, 26, "导出与发布");
    click(&ctx, &mut app, 26, "发布给读者");

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

    click(&ctx, &mut app, 26, "导出与发布");
    click(&ctx, &mut app, 26, "发布给读者");
    click(&ctx, &mut app, 26, "Public Event · event (public)");
    click(&ctx, &mut app, 26, "书稿章节");
    click(&ctx, &mut app, 26, "Public Chapter (public-chapter)");
    click(&ctx, &mut app, 26, "附件");
    click(&ctx, &mut app, 26, "Public Cover (cover)");
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
    let start = rendered
        .find("查看排除明细（")
        .expect("作者必须可查排除报告");
    let end = start + rendered[start..].find('）').unwrap() + '）'.len_utf8();
    let exclusions = rendered[start..end].to_owned();
    let rendered = scroll_from_visible_anchor_to(&ctx, &mut app, 26, "作者只读预览", &exclusions);
    assert!(rendered.contains(&exclusions));
    click(&ctx, &mut app, 26, "继续");
    let rendered = rendered_text_in_window(&ctx, &mut app, 26, "未公开内容");
    assert!(rendered.contains("未公开内容"), "{rendered}");
    click(&ctx, &mut app, 26, "继续");
    set_reader_destination(&ctx, &mut app, &destination);
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
    wait_for_reader_delivery(&ctx, &mut app);
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
        files.keys().any(|path| path.starts_with("assets")
            && path.extension().is_some_and(|extension| extension == "png")),
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

    click(&ctx, &mut app, 26, "导出与发布");
    click(&ctx, &mut app, 26, "发布给读者");
    click(&ctx, &mut app, 26, "Public Event · event (public)");
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);
    click(&ctx, &mut app, 26, "4 确认生成");
    set_reader_destination(&ctx, &mut app, &destination);
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
    // 真实取消按钮→重新打开，不重新点对象；原选择必须仍能直接生成新审核。
    click(&ctx, &mut app, 26, "导出与发布");
    click(&ctx, &mut app, 26, "发布给读者");
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);
    assert!(!destination.exists());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.is_dirty(), dirty);
    assert!(app.history.is_empty());
    click(&ctx, &mut app, 26, "取消发布");
}

#[test]
fn reader_publish_recompile_refreshes_candidates_and_invalidates_the_old_review() {
    let (ctx, mut app) = reader_publish_app();
    let destination = app
        .project
        .root
        .with_file_name(format!("reader-publish-stale-{}.zip", std::process::id()));
    let _ = std::fs::remove_file(&destination);

    click(&ctx, &mut app, 26, "导出与发布");
    click(&ctx, &mut app, 26, "发布给读者");
    click(&ctx, &mut app, 26, "Public Event · event (public)");
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

    click(&ctx, &mut app, 26, "导出与发布");
    click(&ctx, &mut app, 26, "发布给读者");
    click(&ctx, &mut app, 26, "Public Event · event (public)");
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);
    click(&ctx, &mut app, 26, "4 确认生成");
    set_reader_destination(&ctx, &mut app, &destination);
    click(
        &ctx,
        &mut app,
        26,
        "我已逐项核对预览，确认只发布以上离线内容（不代表在线权限控制）",
    );
    click(&ctx, &mut app, 26, "发布 ZIP");
    wait_for_reader_delivery(&ctx, &mut app);
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

#[test]
fn reader_publish_property_checkboxes_are_opt_in_and_actual_zip_keeps_secrets_private() {
    let (ctx, mut app) = reader_publish_app();
    let path = app.active_file.clone();
    let text = format!("character mei as \"梅\"\n  property appearance = \"蓝外套\"\n  property secret = \"READER_FIELD_SECRET_SENTINEL\"\n{}",app.project.document(&path).unwrap());
    app.project.set_text(&path, text).unwrap();
    app.project.save().unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    let destination = app
        .project
        .root
        .with_file_name(format!("reader-fields-ui-{}.zip", std::process::id()));
    let _ = std::fs::remove_file(&destination);
    click(&ctx, &mut app, 26, "导出与发布");
    click(&ctx, &mut app, 26, "发布给读者");
    click(&ctx, &mut app, 26, "梅 · character (mei)");
    click_reader_body(&ctx, &mut app, "appearance：蓝外套");
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);
    click(&ctx, &mut app, 26, "4 确认生成");
    set_reader_destination(&ctx, &mut app, &destination);
    click(
        &ctx,
        &mut app,
        26,
        "我已逐项核对预览，确认只发布以上离线内容（不代表在线权限控制）",
    );
    click(&ctx, &mut app, 26, "发布 ZIP");
    wait_for_reader_delivery(&ctx, &mut app);
    let files = crate::archive::decode(&std::fs::read(&destination).unwrap()).unwrap();
    let bytes = files
        .iter()
        .flat_map(|(path, content)| {
            path.to_string_lossy()
                .bytes()
                .chain(content.iter().copied())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let output = String::from_utf8_lossy(&bytes);
    assert!(output.contains("蓝外套"));
    assert!(!output.contains("READER_FIELD_SECRET_SENTINEL"));
    assert_eq!(baseline, app.project.content_baseline());
    assert!(app.history.is_empty());
    std::fs::remove_file(destination).unwrap();
}

fn set_reader_destination(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    destination: &std::path::Path,
) {
    replace_text_area(
        ctx,
        app,
        26,
        "reader-site.zip",
        &destination.to_string_lossy(),
    );
    assert!(
        rendered_text_in_window(ctx, app, 26, &destination.to_string_lossy())
            .contains(destination.to_string_lossy().as_ref()),
        "必须实际编辑目标路径后才能继续发布"
    );
}

fn wait_for_reader_delivery(ctx: &egui::Context, app: &mut WorldeditApp) {
    for _ in 0..1000 {
        let output = frame(ctx, app, Vec::new(), 26);
        let mut text = String::new();
        for shape in &output.shapes {
            collect_text(&shape.shape, &mut text);
        }
        if text.contains("阅读包已写入") || text.contains("发布失败：") {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("最终交付没有返回成功或错误");
}

fn click_reader_body(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let mut found = None;
    for attempt in 0..80 {
        let output = frame(ctx, app, Vec::new(), 26);
        let bounds = ctx
            .memory(|memory| memory.area_rect(egui::Id::new("reader-publish-window")))
            .unwrap();
        found = output.shapes.iter().find_map(|shape| {
            text_position(&shape.shape, label)
                .filter(|point| bounds.contains(*point) && shape.clip_rect.contains(*point))
        });
        if found.is_some() {
            break;
        }
        // 新底栏固定，滚轮必须放进内容区，不能悬在取消/确认按钮上。
        let hover = pos2(bounds.right() - 48.0, bounds.center().y);
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(hover),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, if attempt < 40 { -100.0 } else { 100.0 }),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            26,
        );
    }
    let point = found.unwrap_or_else(|| panic!("真实阅读向导内容区不可达：{label}"));
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            26,
        );
    }
}
