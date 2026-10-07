//! 完整 App 的滚动几何；检查可见裁剪与实际缩放，不以文本存在代替可操作。
use super::*;
use crate::theme::StylePreset;
mod metadata_guard;
mod small_actions;

fn app_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    physical_size: egui::Vec2,
    events: Vec<Event>,
) -> egui::FullOutput {
    let mut raw = RawInput {
        screen_rect: Some(Rect::from_min_size(
            pos2(0.0, 0.0),
            physical_size / ctx.zoom_factor(),
        )),
        events,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest());
    })
}

fn settle_app(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2) -> egui::FullOutput {
    for _ in 0..5 {
        app_frame(ctx, app, size, vec![]);
    }
    app_frame(ctx, app, size, vec![])
}

fn full_label(output: &egui::FullOutput, label: &str) -> Rect {
    let mut matches = Vec::new();
    for shape in &output.shapes {
        let mut texts = Vec::new();
        text_shapes(&shape.shape, &mut texts);
        for text in texts.into_iter().filter(|text| text.galley.text() == label) {
            let rect = text.galley.rect.translate(text.pos.to_vec2());
            matches.push((rect, shape.clip_rect));
            if shape.clip_rect.contains_rect(rect) {
                return rect;
            }
        }
    }
    panic!("{label} 必须完整可见：{matches:?}");
}

fn app_click(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2, label: &str) {
    let pos = full_label(&settle_app(ctx, app, size), label).center();
    for pressed in [true, false] {
        app_frame(
            ctx,
            app,
            size,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

fn app_key(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2, key: egui::Key) {
    for pressed in [true, false] {
        app_frame(
            ctx,
            app,
            size,
            vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
}

fn wheel(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    pos: egui::Pos2,
    delta: f32,
) {
    app_frame(
        ctx,
        app,
        size,
        vec![
            Event::PointerMoved(pos),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, delta),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    let mut previous = app.manuscript.scroll_y;
    let mut stable = 0;
    for _ in 0..90 {
        app_frame(ctx, app, size, vec![]);
        let current = app.manuscript.scroll_y;
        if (current - previous).abs() < 0.001 {
            stable += 1;
            if stable == 3 {
                return;
            }
        } else {
            stable = 0;
        }
        previous = current;
    }
    panic!("实际滚轮偏移尚未稳定");
}

fn long_titles(app: &mut WorldeditApp) {
    let path = app.project.root.join(".world/manuscripts/book.json");
    let mut book: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&path).unwrap().bytes()).unwrap();
    book["title"] = "非常长的书名需要受限展示同时仍然能够看见章节选择与正文工具"
        .repeat(4)
        .into();
    book["entries"][0]["title"] = "非常长的章节名不能挤掉正文和模式切换应用操作"
        .repeat(4)
        .into();
    app.project
        .set_authoring_document(&path, serde_json::to_vec(&book).unwrap())
        .unwrap();
    app.reset_views();
    app.recompile();
}

#[test]
fn short_focus_restored_scroll_keeps_toolbar_visible_and_has_no_empty_overflow() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    app.personal.settings.style = StylePreset::Focus;
    app.personal.settings.reduce_motion = true;
    app.manuscript.reader_open = false;
    app.manuscript
        .writing_buffers
        .get_mut(&app.active_file)
        .unwrap()
        .replace_source(
            "event start\n  第一段正文。\n  第二段正文。\n  第三段正文。\n  -> END\n".into(),
        );
    let size = vec2(1188.0, 848.0);
    settle_app(&ctx, &mut app, size);
    app.manuscript.pending_scroll = Some(17.0);
    let output = settle_app(&ctx, &mut app, size);
    for label in ["写作", "结构", "源码", "应用正文草稿", "丢弃此文件草稿"] {
        full_label(&output, label);
    }
    assert!(
        app.manuscript.scroll_y.abs() < 0.5,
        "三段短稿应自然归入视口，不能用旧 offset 放大空白：{}",
        app.manuscript.scroll_y
    );
    for _ in 0..5 {
        wheel(&ctx, &mut app, size, pos2(500.0, 450.0), -240.0);
        assert!(app.manuscript.scroll_y.abs() < 0.5, "滚轮不能制造短稿空白");
    }
}

#[test]
fn scaled_small_focus_keeps_short_and_long_body_and_tools_reachable() {
    for paragraphs in [3, 80] {
        let (ctx, mut app) = blank();
        create_start(&ctx, &mut app);
        long_titles(&mut app);
        settle_app(&ctx, &mut app, vec2(1188.0, 848.0));
        app.personal.settings.style = StylePreset::Focus;
        app.personal.settings.reduce_motion = true;
        app.personal.settings.ui_scale = 2.0;
        app.personal.settings.body_size = 28.0;
        app.manuscript.reader_open = false;
        let text = (0..paragraphs)
            .map(|_| "  可见正文。\n")
            .collect::<String>();
        app.manuscript
            .writing_buffers
            .get_mut(&app.active_file)
            .unwrap()
            .replace_source(format!("event start\n{text}  -> END\n"));
        let output = settle_app(&ctx, &mut app, vec2(800.0, 600.0));
        assert_eq!(ctx.zoom_factor(), 2.0);
        assert_eq!(ctx.pixels_per_point(), 2.0);
        assert_eq!(ctx.screen_rect().size(), vec2(400.0, 300.0));
        let mut first_line = None;
        for shape in &output.shapes {
            let mut texts = Vec::new();
            text_shapes(&shape.shape, &mut texts);
            for text in texts {
                if text.galley.text().starts_with("可见正文。") {
                    let rect = text.galley.rows[0].rect().translate(text.pos.to_vec2());
                    first_line = Some((rect, shape.clip_rect));
                }
            }
        }
        assert!(
            first_line.is_some_and(|(rect, clip)| clip.contains_rect(rect)),
            "{paragraphs} 段的首行须完整留在400×300视口：{first_line:?}；{}",
            labels(&output)
        );
        full_label(&output, "正文工具");
        full_label(&output, "选择章节");
        full_label(&output, "书稿工具");
        let size = vec2(800.0, 600.0);
        let baseline = app.project.content_baseline();
        // 从真实可见菜单进入三个编辑模式，再回到写作；仍使用原 TextEdit 身份。
        for (label, mode) in [
            ("结构", crate::app::writing_workspace::Mode::Structure),
            ("源码", crate::app::writing_workspace::Mode::Source),
            ("写作", crate::app::writing_workspace::Mode::Prose),
        ] {
            app_click(&ctx, &mut app, size, "正文工具");
            app_click(&ctx, &mut app, size, label);
            assert_eq!(app.manuscript_session().mode, mode);
        }
        assert_eq!(app.project.content_baseline(), baseline);
        app_click(&ctx, &mut app, size, "正文工具");
        app_click(&ctx, &mut app, size, "应用正文草稿");
        assert!(app
            .project
            .document(&app.active_file)
            .unwrap()
            .contains("可见正文。"));
        settle_app(&ctx, &mut app, size);
        assert!(!app.manuscript.writing_buffers()[0].is_changed());
        // 菜单也可由真实 Tab 焦点和 Enter 召回，不依赖 hover 或手工 request_focus。
        let mut reached = false;
        for _ in 0..48 {
            app_key(&ctx, &mut app, size, egui::Key::Tab);
            let output = settle_app(&ctx, &mut app, size);
            let menu = full_label(&output, "正文工具");
            if ctx
                .memory(|memory| memory.focused())
                .and_then(|id| ctx.read_response(id))
                .is_some_and(|r| r.rect.contains(menu.center()))
            {
                reached = true;
                break;
            }
        }
        assert!(reached, "Tab 必须能够到达正文工具");
        app_key(&ctx, &mut app, size, egui::Key::Enter);
        full_label(&settle_app(&ctx, &mut app, size), "结构");
        app_key(&ctx, &mut app, size, egui::Key::Escape);
    }
}

#[test]
fn long_body_scroll_and_restore_keep_tools_fixed_across_style_preview_cancel() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    app.personal.settings.style = StylePreset::Focus;
    app.personal.settings.reduce_motion = true;
    app.manuscript.reader_open = false;
    let source = format!(
        "event start\n{}  -> END\n",
        "  长篇正文保留滚动与原稿。\n".repeat(100)
    );
    app.manuscript
        .writing_buffers
        .get_mut(&app.active_file)
        .unwrap()
        .replace_source(source.clone());
    let size = vec2(1188.0, 848.0);
    let before = settle_app(&ctx, &mut app, size);
    let toolbar = full_label(&before, "应用正文草稿");
    wheel(&ctx, &mut app, size, pos2(500.0, 450.0), -480.0);
    let output = settle_app(&ctx, &mut app, size);
    assert!(app.manuscript.scroll_y > 200.0, "长稿必须真实滚动");
    assert_eq!(full_label(&output, "应用正文草稿"), toolbar);
    let saved = app.manuscript_session();
    let offset = saved.scroll_y;
    assert_eq!(
        app.manuscript.validate_session(&app.project, &saved),
        Ok(true)
    );
    app.restore_manuscript_session(saved);
    settle_app(&ctx, &mut app, size);
    assert!(
        (app.manuscript.scroll_y - offset).abs() < 0.5,
        "恢复正文位置：{} / {offset}",
        app.manuscript.scroll_y
    );
    let baseline = app.project.content_baseline();
    let prefs = (
        app.personal.settings.focus,
        app.manuscript.reader_open,
        app.manuscript.narrow_preview,
    );
    for label in [
        "Studio 编辑台",
        "Manuscript 稿纸",
        "Technical 工具台",
        "Ledger 档案",
    ] {
        app_click(&ctx, &mut app, size, "外观");
        app_click(&ctx, &mut app, size, label);
        assert_ne!(app.personal.appearance().style, StylePreset::Focus);
        app_click(&ctx, &mut app, size, "取消");
        let output = settle_app(&ctx, &mut app, size);
        assert_eq!(app.personal.appearance().style, StylePreset::Focus);
        assert!(
            (app.manuscript.scroll_y - offset).abs() < 0.5,
            "{label} 取消丢失滚动：{} / {offset}",
            app.manuscript.scroll_y
        );
        assert_eq!(full_label(&output, "应用正文草稿"), toolbar);
        assert_eq!(
            (
                app.personal.settings.focus,
                app.manuscript.reader_open,
                app.manuscript.narrow_preview
            ),
            prefs
        );
        assert_eq!(app.manuscript.writing_buffers()[0].source(), source);
        assert_eq!(app.project.content_baseline(), baseline);
    }
}

#[test]
fn returning_to_section_does_not_send_its_empty_offset_to_a_long_chapter() {
    for missing_source in [false, true] {
        let (ctx, mut app) = blank();
        create_start(&ctx, &mut app);
        click(&ctx, &mut app, "插入分节");
        click(&ctx, &mut app, "应用书稿");
        click(&ctx, &mut app, "潮汐初起");
        if missing_source {
            let local = app.manuscript.books.get_mut("book").unwrap();
            let section = local
                .draft
                .entries
                .iter_mut()
                .find(|entry| entry.id == "section")
                .unwrap();
            section.kind = ManuscriptEntryKind::Chapter;
            section.target_ref = Some(TargetRef::new("event", "missing"));
            section.title = "无可用来源".into();
            local.changed = true;
        }
        app.personal.settings.style = StylePreset::Focus;
        app.personal.settings.reduce_motion = true;
        app.manuscript.reader_open = false;
        let source = format!(
            "event start\n{}  -> END\n",
            "  长章滚动位置。\n".repeat(100)
        );
        app.manuscript
            .writing_buffers
            .get_mut(&app.active_file)
            .unwrap()
            .replace_source(source.clone());
        let size = vec2(1188.0, 848.0);
        settle_app(&ctx, &mut app, size);
        wheel(&ctx, &mut app, size, pos2(500.0, 450.0), -480.0);
        // Click the visible middle of the document so a later focus request keeps
        // a real mid-document caret, rather than correctly returning to character 0.
        for pressed in [true, false] {
            app_frame(
                &ctx,
                &mut app,
                size,
                vec![
                    Event::PointerMoved(pos2(500.0, 450.0)),
                    Event::PointerButton {
                        pos: pos2(500.0, 450.0),
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        settle_app(&ctx, &mut app, size);
        let offset = app.manuscript.scroll_y;
        assert!(offset > 200.0);
        app_click(&ctx, &mut app, size, "选择章节");
        app_click(
            &ctx,
            &mut app,
            size,
            if missing_source {
                "无可用来源"
            } else {
                "分节 · 新分节"
            },
        );
        let section = app.manuscript_session();
        assert!(section.anchor.is_none());
        app.tab = Tab::Edit;
        app_frame(&ctx, &mut app, size, vec![]);
        app.restore_manuscript_session(section);
        app.tab = Tab::Manuscript;
        settle_app(&ctx, &mut app, size);
        assert!(
            app.manuscript.pending_scroll.is_none(),
            "无正文分节应消费自身恢复请求"
        );
        app_click(&ctx, &mut app, size, "选择章节");
        app_click(&ctx, &mut app, size, "潮汐初起");
        settle_app(&ctx, &mut app, size);
        assert!(
            (app.manuscript.scroll_y - offset).abs() < 0.5,
            "章节的真实滚动位置：{} / {offset}",
            app.manuscript.scroll_y
        );
        assert_eq!(app.manuscript.writing_buffers()[0].source(), source);
    }
}

#[test]
fn resizing_open_metadata_keeps_each_real_field_ime_receiver() {
    for role in ["book", "title", "summary", "status", "goal"] {
        let (ctx, mut app) = blank();
        create_start(&ctx, &mut app);
        app.personal.settings.reduce_motion = true;
        app.manuscript.reader_open = false;
        let wide = vec2(1600.0, 1500.0);
        app_click(&ctx, &mut app, wide, "编排与来源");
        settle_app(&ctx, &mut app, wide);
        let id = if role == "book" {
            egui::Id::new(("manuscript-book-title", &app.project.root, "book"))
        } else {
            egui::Id::new((
                "manuscript-entry-field",
                &app.project.root,
                "book",
                "chapter",
                role,
            ))
        };
        let response = ctx.read_response(id).unwrap();
        assert!(
            response.interact_rect.contains_rect(response.rect),
            "字段必须真实可点击：{role} {response:?}"
        );
        let position = response.rect.center();
        for pressed in [true, false] {
            app_frame(
                &ctx,
                &mut app,
                wide,
                vec![
                    Event::PointerMoved(position),
                    Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(id),
            "{role} 焦点"
        );
        let baseline = app.project.content_baseline();
        app_frame(
            &ctx,
            &mut app,
            wide,
            vec![
                Event::Ime(egui::ImeEvent::Enabled),
                Event::Ime(egui::ImeEvent::Preedit("待提交".into())),
            ],
        );
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(id),
            "宽窗首Preedit仍归原框：{role}"
        );
        app_frame(
            &ctx,
            &mut app,
            vec2(520.0, 600.0),
            vec![Event::Ime(egui::ImeEvent::Commit("正式输入".into()))],
        );
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(id),
            "同帧缩窗Commit仍归原框：{role}"
        );
        let draft = &app.manuscript.books.get("book").unwrap().draft;
        let entry = &draft.entries[0];
        let text = match role {
            "book" => draft.title.as_str(),
            "title" => entry.title.as_str(),
            "summary" => entry.summary.as_deref().unwrap(),
            "status" => entry.status.as_deref().unwrap(),
            "goal" => entry.goal.as_deref().unwrap(),
            _ => unreachable!(),
        };
        assert!(
            text.contains("正式输入") && !text.contains("待提交"),
            "{role}: {text}"
        );
        assert_eq!(app.project.content_baseline(), baseline);
    }
}

#[test]
fn short_manuscript_stays_bounded_in_all_five_styles() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    app.personal.settings.reduce_motion = true;
    app.manuscript.reader_open = false;
    app.manuscript
        .writing_buffers
        .get_mut(&app.active_file)
        .unwrap()
        .replace_source(
            "event start\n  第一段正文。\n  第二段正文。\n  第三段正文。\n  -> END\n".into(),
        );
    let size = vec2(1188.0, 848.0);
    for style in [
        StylePreset::Ledger,
        StylePreset::Studio,
        StylePreset::Manuscript,
        StylePreset::Focus,
        StylePreset::Technical,
    ] {
        app.personal.settings.style = style;
        settle_app(&ctx, &mut app, size);
        app.manuscript.pending_scroll = Some(17.0);
        let output = settle_app(&ctx, &mut app, size);
        assert!(
            app.manuscript.scroll_y.abs() < 0.5,
            "{style:?}: {}",
            app.manuscript.scroll_y
        );
        if labels(&output).contains("正文工具") {
            full_label(&output, "正文工具");
        } else {
            full_label(&output, "应用正文草稿");
        }
    }
}
