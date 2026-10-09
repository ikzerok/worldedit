//! 763×541 物理窗口 + 产品自身 200% 缩放，不能用 763×541 逻辑点代替。
use super::*;

const PHYSICAL: egui::Vec2 = vec2(763.0, 541.0);

fn native_budget_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(
            pos2(0.0, 0.0),
            PHYSICAL / ctx.pixels_per_point(),
        )),
        events,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut input);
    ctx.run(input, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest());
    })
}

fn settle(ctx: &egui::Context, app: &mut WorldeditApp) -> egui::FullOutput {
    for _ in 0..5 {
        native_budget_frame(ctx, app, vec![]);
    }
    let output = native_budget_frame(ctx, app, vec![]);
    assert_eq!(ctx.zoom_factor(), 2.0);
    assert_eq!(ctx.pixels_per_point(), 2.0);
    assert_eq!(ctx.screen_rect().size(), vec2(381.5, 270.5));
    output
}

fn narrow_fixture(kind: Kind) -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = fixture();
    app.begin_manuscript_world_links(&ctx);
    new_character(&mut app);
    let state = app.manuscript.world_links.as_mut().unwrap();
    state.kind = kind;
    state.entity_type = "place".into();
    state.description = "灯塔旁的世界资料。".into();
    state.enable_entities = kind == Kind::Entity;
    app.personal.settings.ui_scale = 2.0;
    app.personal.settings.reduce_motion = true;
    settle(&ctx, &mut app);
    (ctx, app)
}

fn visible_label(ctx: &egui::Context, output: &egui::FullOutput, label: &str) -> Option<Rect> {
    for shape in &output.shapes {
        let mut texts = Vec::new();
        text_shapes(&shape.shape, &mut texts);
        for text in texts.into_iter().filter(|text| text.galley.text() == label) {
            let rect = text.galley.rect.translate(text.pos.to_vec2());
            if ctx.screen_rect().contains_rect(rect) && shape.clip_rect.contains_rect(rect) {
                return Some(rect);
            }
        }
    }
    None
}

fn complete_label(ctx: &egui::Context, output: &egui::FullOutput, label: &str) -> Rect {
    visible_label(ctx, output, label)
        .unwrap_or_else(|| panic!("{label} 的全部文面必须在真实视口内：{}", labels(output)))
}

fn complete_control(ctx: &egui::Context, id: egui::Id) -> egui::Response {
    let response = ctx.read_response(id).expect("控件应实际绘制");
    assert!(
        ctx.screen_rect().contains_rect(response.rect)
            && response.interact_rect.contains_rect(response.rect),
        "控件须全部可见可操作：{id:?}, rect={:?}, interact={:?}, screen={:?}",
        response.rect,
        response.interact_rect,
        ctx.screen_rect()
    );
    response
}

fn click_label(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) -> egui::Id {
    let output = settle(ctx, app);
    let pos = complete_label(ctx, &output, label).center();
    for pressed in [true, false] {
        native_budget_frame(
            ctx,
            app,
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
    ctx.interaction_snapshot(|snapshot| snapshot.clicked)
        .expect("须实际点击控件")
}

fn wheel(ctx: &egui::Context, app: &mut WorldeditApp, delta: f32) -> egui::FullOutput {
    let pos = pos2(
        ctx.screen_rect().center().x,
        ctx.screen_rect().bottom() - 30.0,
    );
    native_budget_frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(pos),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, delta),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..60 {
        native_budget_frame(ctx, app, vec![]);
    }
    settle(ctx, app)
}

fn tab(ctx: &egui::Context, app: &mut WorldeditApp, reverse: bool) {
    for pressed in [true, false] {
        native_budget_frame(
            ctx,
            app,
            vec![Event::Key {
                key: egui::Key::Tab,
                physical_key: Some(egui::Key::Tab),
                pressed,
                repeat: false,
                modifiers: if reverse {
                    egui::Modifiers::SHIFT
                } else {
                    egui::Modifiers::NONE
                },
            }],
        );
    }
    settle(ctx, app);
}

fn field(label: &str) -> egui::Id {
    egui::Id::new(("world-link-field", label))
}

#[test]
fn physical_763_by_541_at_product_200_percent_shows_whole_destination_and_plan_end() {
    for kind in [Kind::Character, Kind::Entity] {
        let (ctx, mut app) = narrow_fixture(kind);
        let baseline = app.project.content_baseline();
        let history = app.history.len();
        let source = app.manuscript.writing_buffers[&app.project.entry]
            .source()
            .to_owned();
        let output = settle(&ctx, &mut app);
        for label in ["返回正文，保留输入", "预览关联计划", "更多"] {
            complete_label(&ctx, &output, label);
        }
        wheel(&ctx, &mut app, -2000.0);
        if kind == Kind::Entity {
            // 目标文件在迁移说明之前；用真实滚轮找到完整文字，再检查整个控件。
            for _ in 0..16 {
                let output = settle(&ctx, &mut app);
                if visible_label(&ctx, &output, "资料.wl").is_some() {
                    // 留出控件边框，而非只使文字勉强可见。
                    wheel(&ctx, &mut app, 16.0);
                    break;
                }
                wheel(&ctx, &mut app, 24.0);
            }
        }
        let combo = click_label(&ctx, &mut app, "资料.wl");
        complete_control(&ctx, combo);
        click_label(&ctx, &mut app, "world.wl");
        assert_eq!(
            app.manuscript.world_links.as_ref().unwrap().destination,
            app.project.entry
        );
        let preview = click_label(&ctx, &mut app, "预览关联计划");
        complete_control(&ctx, preview);
        let output = settle(&ctx, &mut app);
        assert!(
            app.manuscript
                .world_links
                .as_ref()
                .unwrap()
                .plan
                .as_ref()
                .unwrap()
                .can_apply
        );
        for label in ["返回正文，保留输入", "预览关联计划", "应用这组关联草稿"]
        {
            complete_label(&ctx, &output, label);
        }
        let output = wheel(&ctx, &mut app, -10000.0);
        complete_label(&ctx, &output, "应用不写磁盘；保存后重开仍按稳定身份关联。");
        click_label(&ctx, &mut app, "返回正文，保留输入");
        assert!(!app.manuscript.world_links.as_ref().unwrap().open);
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(app.project.language_version(), "1.9");
        assert_eq!(app.history.len(), history);
        assert_eq!(
            app.manuscript.writing_buffers[&app.project.entry].source(),
            source
        );
    }
}

#[test]
fn physical_small_tab_and_shift_tab_reveal_whole_fields_without_later_pointer_pullback() {
    let (ctx, mut app) = narrow_fixture(Kind::Entity);
    let baseline = app.project.content_baseline();
    ctx.memory_mut(|memory| memory.request_focus(field("稳定 ID")));
    native_budget_frame(&ctx, &mut app, vec![]);
    for label in ["显示名称", "实体分类", "资料说明"] {
        tab(&ctx, &mut app, false);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(field(label)));
        complete_control(&ctx, field(label));
    }
    for label in ["实体分类", "显示名称", "稳定 ID"] {
        tab(&ctx, &mut app, true);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(field(label)));
        complete_control(&ctx, field(label));
    }
    let visible = complete_control(&ctx, field("稳定 ID")).rect;
    wheel(&ctx, &mut app, -2000.0);
    let after_pointer = ctx.read_response(field("稳定 ID")).unwrap().rect;
    assert!(after_pointer.top() < visible.top() - 40.0, "新滚轮应生效");
    settle(&ctx, &mut app);
    let later = ctx.read_response(field("稳定 ID")).unwrap().rect;
    assert!(
        (later.top() - after_pointer.top()).abs() < 0.5,
        "旧焦点不能拉回文面"
    );
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        Some(field("稳定 ID"))
    );
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn compact_more_keeps_explicit_selection_change_and_confirmed_clear_reachable() {
    let (ctx, mut app) = narrow_fixture(Kind::Character);
    let baseline = app.project.content_baseline();
    click_label(&ctx, &mut app, "更多");
    let output = settle(&ctx, &mut app);
    complete_label(&ctx, &output, "改用当前选区");
    click_label(&ctx, &mut app, "清除输入…");
    let output = settle(&ctx, &mut app);
    complete_label(&ctx, &output, "确认清除关联输入");
    assert!(app.manuscript.world_links.as_ref().unwrap().touched);
    click_label(&ctx, &mut app, "确认清除关联输入");
    assert!(app.manuscript.world_links.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
}
