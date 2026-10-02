//! 固定表单操作区与长内容滚动的真实 egui 绘制回归。
use super::*;

fn form_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| app.entity_editor_window(ctx),
    )
}

fn visible_rect(output: &egui::FullOutput, label: &str) -> Option<Rect> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(text) if text.galley.text() == label => {
            let rect = text.galley.rect.translate(text.pos.to_vec2());
            shape.clip_rect.contains_rect(rect).then_some(rect)
        }
        _ => None,
    })
}

#[test]
fn long_entity_form_keeps_actions_fixed_with_large_text_and_stale_status() {
    for (size, scale) in [
        (vec2(1040.0, 660.0), 1.0),
        (vec2(1188.0, 848.0), 1.0),
        (vec2(1188.0, 848.0), 1.5),
    ] {
        let (ctx, mut app) = app();
        ctx.style_mut(|style| {
            style.animation_time = 0.0;
            for font in style.text_styles.values_mut() {
                font.size *= scale;
            }
        });
        app.edit_entity(Some("a"));
        let form = app.entity_editor.as_mut().unwrap();
        form.draft.description = "这是保留的长资料正文。\n".repeat(160);
        form.draft.display = "尚未应用的长资料".into();
        let baseline = app.project.content_baseline();
        for _ in 0..8 {
            form_frame(&ctx, &mut app, size, vec![]);
        }
        let first = form_frame(&ctx, &mut app, size, vec![]);
        let apply = visible_rect(&first, "应用资料").expect("长正文顶部应可应用");
        let cancel = visible_rect(&first, "取消").expect("长正文顶部应可取消");
        let window = ctx
            .memory(|memory| memory.area_rect(egui::Id::new("entity-editor")))
            .unwrap();
        assert!(Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(window));
        for _ in 0..8 {
            let output = form_frame(
                &ctx,
                &mut app,
                size,
                vec![
                    Event::PointerMoved(window.center()),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, -500.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            assert_eq!(visible_rect(&output, "应用资料"), Some(apply));
            assert_eq!(visible_rect(&output, "取消"), Some(cancel));
        }
        app.recompile();
        for _ in 0..8 {
            form_frame(&ctx, &mut app, size, vec![]);
        }
        let output = form_frame(&ctx, &mut app, size, vec![]);
        assert!(visible_rect(
            &output,
            "工程已变化。输入已保留；请复制所需内容并重新打开表单后合并。"
        )
        .is_some());
        let point = visible_rect(&output, "应用资料").unwrap().center();
        for pressed in [true, false] {
            form_frame(
                &ctx,
                &mut app,
                size,
                vec![
                    Event::PointerMoved(point),
                    Event::PointerButton {
                        pos: point,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert_eq!(app.project.content_baseline(), baseline);
        assert!(app.history.is_empty());
        assert_eq!(
            app.entity_editor.as_ref().unwrap().draft.display,
            "尚未应用的长资料"
        );
        assert!(visible_rect(&form_frame(&ctx, &mut app, size, vec![]), "取消").is_some());
    }
}

#[test]
fn entity_capability_status_and_cancel_remain_visible_below_long_content() {
    let (ctx, mut app) = app();
    app.edit_entity(Some("a"));
    app.entity_editor.as_mut().unwrap().draft.description = "保留正文\n".repeat(160);
    let manifest = app.project.root.join(".world/project.json");
    app.project
        .set_authoring_document(
            &manifest,
            br#"{"schema_version":1,"language_version":"1.9","required_features":[]}"#.to_vec(),
        )
        .unwrap();
    let baseline = app.project.content_baseline();
    let size = vec2(1040.0, 660.0);
    for _ in 0..8 {
        form_frame(&ctx, &mut app, size, vec![]);
    }
    let output = form_frame(&ctx, &mut app, size, vec![]);
    assert!(visible_rect(
        &output,
        "能力只读：通用实体需要语言 1.10 与 content.entities.v1；不会自动迁移。"
    )
    .is_some());
    assert!(visible_rect(&output, "查看语言与资料能力…").is_some());
    let point = visible_rect(&output, "取消").unwrap().center();
    for pressed in [true, false] {
        form_frame(
            &ctx,
            &mut app,
            size,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert!(app.entity_editor.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}
