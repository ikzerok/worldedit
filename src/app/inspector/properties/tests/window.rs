use super::*;

struct WindowFrame {
    output: egui::FullOutput,
    rect: Rect,
    apply: Rect,
}
fn window_frame(form: &mut Form, open: &mut bool, events: Vec<Event>) -> WindowFrame {
    let mut rect = Rect::NOTHING;
    let mut apply = Rect::NOTHING;
    let output = form.ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1188.0, 848.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            // No max_width: intrinsic content must not grow a resizable Window.
            if let Some(response) = egui::Window::new("人物引用窗口")
                .id(egui::Id::new("reference-width-window"))
                .default_pos(pos2(88.0, 88.0))
                .default_width(620.0)
                .resizable(true)
                .open(open)
                .show(ctx, |ui| {
                    egui::CollapsingHeader::new("自定义属性")
                        .default_open(true)
                        .show(ui, |ui| {
                            properties_with_references(
                                ui,
                                &mut form.values,
                                &form.catalog,
                                form.options,
                            );
                        });
                    apply = ui.button("应用资料").rect;
                })
            {
                rect = response.response.rect;
            }
        },
    );
    WindowFrame {
        output,
        rect,
        apply,
    }
}
fn window_click(form: &mut Form, open: &mut bool, text: &str) {
    for _ in 0..3 {
        window_frame(form, open, Vec::new());
    }
    let frame = window_frame(form, open, Vec::new());
    let point = frame
        .output
        .shapes
        .iter()
        .find_map(|shape| find(&shape.shape, text))
        .unwrap_or_else(|| panic!("找不到{text}"));
    for pressed in [true, false] {
        window_frame(
            form,
            open,
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
fn assert_stable(form: &mut Form, open: &mut bool, expected_width: f32) {
    let viewport = Rect::from_min_size(pos2(0.0, 0.0), vec2(1188.0, 848.0));
    for _ in 0..45 {
        let frame = window_frame(form, open, Vec::new());
        assert!(
            frame.rect.width() <= expected_width + 2.0,
            "Window feedback growth: {:?}, initial={expected_width}",
            frame.rect
        );
        assert!(
            viewport.contains_rect(frame.rect),
            "titlebar/X outside viewport: {:?}",
            frame.rect
        );
        assert!(
            viewport.contains_rect(frame.apply),
            "bottom action outside viewport: {:?}",
            frame.apply
        );
    }
}

#[test]
fn reference_only_and_reference_plus_string_windows_stay_bounded_over_many_frames() {
    for include_text in [false, true] {
        let mut form = Form::new(
            CompileOptions::v1_13()
                .with_object_refs(true)
                .with_character_refs(true),
        );
        form.values = vec![(
            "captain".into(),
            PropertyValue::Ref(TargetRef::new("character", "lin")),
        )];
        if include_text {
            form.values
                .push(("note".into(), PropertyValue::Str("普通短字符串".into())));
        }
        let mut open = true;
        for _ in 0..3 {
            window_frame(&mut form, &mut open, Vec::new());
        }
        let width = window_frame(&mut form, &mut open, Vec::new()).rect.width();
        assert!(width < 700.0, "initial width={width}");
        assert_stable(&mut form, &mut open, width);
    }
}

#[test]
fn long_reference_caption_and_source_wrap_without_growing_window_on_popup_escape() {
    let mut form = Form::new(
        CompileOptions::v1_13()
            .with_object_refs(true)
            .with_character_refs(true),
    );
    let display = "长人物显示名".repeat(35);
    let source = format!("/workspace/{}/人物.wl", "long-source-directory/".repeat(22));
    form.catalog.objects[0].display = display.clone();
    form.catalog.objects[0].file = source.clone();
    form.values = vec![
        (
            "captain".into(),
            PropertyValue::Ref(TargetRef::new("character", "lin")),
        ),
        ("note".into(), PropertyValue::Str("短字符串".into())),
    ];
    let original = form.values.clone();
    let caption = format!("{display} · character:lin");
    let candidate = format!("{display} · 人物:lin\n{source}:1");
    let mut open = true;
    for _ in 0..3 {
        window_frame(&mut form, &mut open, Vec::new());
    }
    let width = window_frame(&mut form, &mut open, Vec::new()).rect.width();
    assert!(
        width < 700.0,
        "long caption expanded closed Window: {width}"
    );
    assert_stable(&mut form, &mut open, width);
    window_click(&mut form, &mut open, &caption);
    assert!(egui::Popup::is_any_open(&form.ctx));
    let frame = window_frame(&mut form, &mut open, Vec::new());
    assert!(
        has(&frame.output, &candidate),
        "full identity/source must remain in wrapped candidate"
    );
    assert_stable(&mut form, &mut open, width);
    window_frame(
        &mut form,
        &mut open,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(!egui::Popup::is_any_open(&form.ctx));
    assert_eq!(form.values, original);
    assert_stable(&mut form, &mut open, width);
    window_click(&mut form, &mut open, &caption);
    window_click(&mut form, &mut open, &candidate);
    assert_eq!(form.values, original);
    assert_stable(&mut form, &mut open, width);
}
