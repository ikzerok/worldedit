use super::*;

fn large_state() -> ReaderPublishState {
    let mut state = ReaderPublishState::new();
    state.object_choices = (0..2000)
        .map(|index| ObjectChoice {
            target: TargetRef::new("entity", &format!("e{index:04}")),
            display: format!("对象{index:04}"),
            aliases: vec![format!("别名{index:04}")],
            fields: (0..10)
                .map(
                    |field| worldline_core::reader_export::ReaderFieldCandidate {
                        key: format!("field{field}"),
                        preview: "未授权属性内容".repeat(20),
                    },
                )
                .collect(),
        })
        .collect();
    state
}
fn draw(
    ctx: &egui::Context,
    state: &mut ReaderPublishState,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1188.0, 848.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    state.object_selection_ui(ui);
                    state.unavailable_ui(ui);
                });
            });
        },
    )
}
fn position(shape: &egui::Shape, text: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(shape) if shape.galley.job.text == text => {
            Some(shape.pos + shape.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| position(shape, text)),
        _ => None,
    }
}
fn click(ctx: &egui::Context, state: &mut ReaderPublishState, label: &str) {
    for _ in 0..3 {
        draw(ctx, state, vec![]);
    }
    let output = draw(ctx, state, vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| position(&shape.shape, label))
        .expect(label);
    for pressed in [true, false] {
        draw(
            ctx,
            state,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

#[test]
fn two_thousand_candidates_remain_reachable_and_bulk_selection_spans_every_page() {
    let ctx = egui::Context::default();
    crate::fonts::install_cjk_fonts(&ctx);
    crate::theme::configure(&ctx, crate::theme::ThemeMode::Dark);
    let mut state = large_state();
    click(&ctx, &mut state, "下一页");
    assert_eq!(state.object_page, 1);
    click(&ctx, &mut state, "选择全部筛选资料");
    assert_eq!(state.objects.len(), 2000, "不能只选当前100项");
    assert!(state.fields.is_empty(), "批量对象不授权属性");
    state.query = "别名1999".into();
    click(&ctx, &mut state, "取消全部筛选资料");
    assert_eq!(state.objects.len(), 1999);
    assert!(!state.objects.contains(&TargetRef::new("entity", "e1999")));
    assert!(state.objects.contains(&TargetRef::new("entity", "e0000")));
}

#[cfg(not(debug_assertions))]
#[test]
fn two_thousand_candidate_selection_meets_release_frame_budget() {
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = crate::app::WorldeditApp::new(&creation, None);
    app.reader_publish = large_state();
    app.reader_publish.open = true;
    app.reader_publish.objects = app
        .reader_publish
        .object_choices
        .iter()
        .map(|choice| choice.target.clone())
        .collect();
    let draw_window = |app: &mut crate::app::WorldeditApp| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1188.0, 848.0),
                )),
                ..Default::default()
            },
            |ctx| app.reader_publish_window(ctx),
        )
    };
    for _ in 0..10 {
        draw_window(&mut app);
    }
    let mut samples = Vec::new();
    for _ in 0..160 {
        let started = std::time::Instant::now();
        draw_window(&mut app);
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    let p95 = samples[151];
    let max = *samples.last().unwrap();
    println!("READER_SELECTION_2000 frame_p95_ms={p95:.3} frame_max_ms={max:.3}");
    assert!(p95 <= 33.0, "2000候选暖帧P95超33ms：{p95}");
    assert!(max <= 250.0, "2000候选单帧超250ms：{max}");
}
