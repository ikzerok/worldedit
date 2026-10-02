use super::*;

fn bounded_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1188.0, 848.0))),
            events,
            ..Default::default()
        },
        |ctx| app.entity_editor_window(ctx),
    )
}
fn click_label(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for _ in 0..3 {
        bounded_frame(ctx, app, Vec::new());
    }
    let output = bounded_frame(ctx, app, Vec::new());
    let point = output
        .shapes
        .iter()
        .find_map(|shape| clipped_text_position(&shape.shape, label, shape.clip_rect))
        .unwrap_or_else(|| panic!("找不到{label}"));
    for pressed in [true, false] {
        bounded_frame(
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
        );
    }
}
#[test]
fn real_entity_window_expanded_character_refs_remain_in_viewport_after_sixty_frames() {
    let (ctx, mut app) = app();
    let manifest = app.project.root.join(".world/project.json");
    app.project.set_authoring_document(&manifest,br#"{"schema_version":1,"language_version":"1.13","required_features":["content.object_refs.v1","content.character_refs.v1"]}"#.to_vec()).unwrap();
    let path = app.active_file.clone();
    app.project.set_text(&path,"character regent as \"摄政王\"\nentity harbor kind place as \"港口\"\n  property commander = ref(\"character\", \"regent\")\n  property note = \"普通字符串\"\nevent start\n  -> END\n".into()).unwrap();
    app.recompile();
    app.edit_entity(Some("harbor"));
    click_label(&ctx, &mut app, "自定义属性");
    let viewport = Rect::from_min_size(pos2(0.0, 0.0), vec2(1188.0, 848.0));
    let mut rectangles = Vec::new();
    for _ in 0..60 {
        bounded_frame(&ctx, &mut app, Vec::new());
        rectangles.push(
            ctx.memory(|memory| memory.area_rect(egui::Id::new("entity-editor")))
                .unwrap(),
        );
    }
    // Include the complete trace on failure; only the first few layout passes may settle.
    let initial = rectangles[5];
    for rect in &rectangles[5..] {
        assert!(
            rect.width() <= initial.width() + 2.0,
            "entity form cumulative growth: {rectangles:?}"
        );
        assert!(
            viewport.contains_rect(*rect),
            "entity titlebar/X escaped viewport: {rectangles:?}"
        );
    }
    // The fixed footer stays visible before and throughout content scrolling.
    let before_scroll = bounded_frame(&ctx, &mut app, Vec::new());
    assert!(visible_text_position(&before_scroll, "应用资料").is_some());
    let mut found = false;
    for _ in 0..20 {
        let output = bounded_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(initial.center()),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -100.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        found |= output.shapes.iter().any(|shape| {
            clipped_text_position(&shape.shape, "应用资料", shape.clip_rect).is_some()
        });
        if found {
            break;
        }
    }
    assert!(found, "固定应用动作必须保持在视口内");
    assert!(app.entity_editor.is_some());
}
