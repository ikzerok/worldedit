use super::{Prototype, Task};
use egui::{pos2, vec2, Event, ImeEvent, PointerButton, RawInput, Rect};

pub(super) fn frame(
    ctx: &egui::Context,
    prototype: &mut Prototype,
    events: Vec<Event>,
    size: [f32; 2],
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(size[0], size[1]))),
            events,
            ..Default::default()
        },
        |ctx| prototype.draw(ctx),
    )
}

fn frame_with_native_scale(
    ctx: &egui::Context,
    prototype: &mut Prototype,
    size: [f32; 2],
    native_pixels_per_point: f32,
) -> egui::FullOutput {
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(size[0], size[1]))),
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .expect("root viewport")
        .native_pixels_per_point = Some(native_pixels_per_point);
    ctx.run(input, |ctx| prototype.draw(ctx))
}

fn text_position(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text == label => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| text_position(shape, label)),
        _ => None,
    }
}

fn contains_text(shape: &egui::Shape, label: &str) -> bool {
    match shape {
        egui::Shape::Text(text) => text.galley.job.text.contains(label),
        egui::Shape::Vec(shapes) => shapes.iter().any(|shape| contains_text(shape, label)),
        _ => false,
    }
}

fn click_at(ctx: &egui::Context, prototype: &mut Prototype, size: [f32; 2], point: egui::Pos2) {
    for pressed in [true, false] {
        let _ = frame(
            ctx,
            prototype,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            size,
        );
    }
}

pub(super) fn click(ctx: &egui::Context, prototype: &mut Prototype, size: [f32; 2], label: &str) {
    for _ in 0..4 {
        let _ = frame(ctx, prototype, Vec::new(), size);
    }
    let output = frame(ctx, prototype, Vec::new(), size);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position(&shape.shape, label))
        .unwrap_or_else(|| panic!("原型控件未显示：{label}"));
    click_at(ctx, prototype, size, point);
}

pub(super) fn click_last(
    ctx: &egui::Context,
    prototype: &mut Prototype,
    size: [f32; 2],
    label: &str,
) {
    for _ in 0..4 {
        let _ = frame(ctx, prototype, Vec::new(), size);
    }
    let output = frame(ctx, prototype, Vec::new(), size);
    let point = output
        .shapes
        .iter()
        .filter_map(|shape| text_position(&shape.shape, label))
        .next_back()
        .unwrap_or_else(|| panic!("原型控件未显示：{label}"));
    click_at(ctx, prototype, size, point);
}

fn drag_slider_left_of_label(
    ctx: &egui::Context,
    prototype: &mut Prototype,
    size: [f32; 2],
    label: &str,
    start_offset: f32,
    end_offset: f32,
) {
    for _ in 0..4 {
        let _ = frame(ctx, prototype, Vec::new(), size);
    }
    let output = frame(ctx, prototype, Vec::new(), size);
    let label_pos = output
        .shapes
        .iter()
        .find_map(|shape| text_position(&shape.shape, label))
        .unwrap_or_else(|| panic!("滑块标签未显示：{label}"));
    let start = label_pos + vec2(start_offset, 0.0);
    let end = label_pos + vec2(end_offset, 0.0);
    let _ = frame(
        ctx,
        prototype,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        size,
    );
    let _ = frame(ctx, prototype, vec![Event::PointerMoved(end)], size);
    let _ = frame(
        ctx,
        prototype,
        vec![
            Event::PointerMoved(end),
            Event::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        size,
    );
}

fn text(ctx: &egui::Context, prototype: &mut Prototype, size: [f32; 2], value: &str) {
    let _ = frame(ctx, prototype, vec![Event::Text(value.into())], size);
}

fn ime(ctx: &egui::Context, prototype: &mut Prototype, size: [f32; 2], event: ImeEvent) {
    let _ = frame(ctx, prototype, vec![Event::Ime(event)], size);
}

fn escape(ctx: &egui::Context, prototype: &mut Prototype, size: [f32; 2]) {
    let _ = frame(
        ctx,
        prototype,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        size,
    );
}

#[test]
fn temporary_reader_returns_keyboard_focus_to_ime_draft() {
    let ctx = egui::Context::default();
    let mut prototype = Prototype::default();
    let size = [1024.0, 640.0];
    let draft_id = egui::Id::new(("eds11_draft", 0));

    click(&ctx, &mut prototype, size, "J2 写作旁查");
    assert!(prototype.task == Task::Writing);

    click(
        &ctx,
        &mut prototype,
        size,
        "在此输入未应用草稿，切任务/工程应明确保留",
    );
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(draft_id));
    ime(&ctx, &mut prototype, size, ImeEvent::Enabled);
    ime(&ctx, &mut prototype, size, ImeEvent::Preedit("雾港".into()));
    assert_eq!(prototype.drafts[0], "雾港");
    assert!(prototype.task == Task::Writing && !prototype.drawer);
    ime(&ctx, &mut prototype, size, ImeEvent::Commit("雾港".into()));
    assert_eq!(prototype.drafts[0], "雾港");

    click(&ctx, &mut prototype, size, "临时旁查 B");
    assert!(prototype.drawer);
    escape(&ctx, &mut prototype, size);
    assert!(!prototype.drawer);
    assert_eq!(prototype.drafts[0], "雾港");
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(draft_id));
    text(&ctx, &mut prototype, size, "继续");
    assert_eq!(prototype.drafts[0], "雾港继续");
}

#[test]
fn temporary_reader_restores_focus_after_a_held_pointer_click() {
    let ctx = egui::Context::default();
    let mut prototype = Prototype::default();
    let size = [1024.0, 640.0];
    let draft_id = egui::Id::new(("eds11_draft", 0));
    click(&ctx, &mut prototype, size, "J2 写作旁查");
    click(
        &ctx,
        &mut prototype,
        size,
        "在此输入未应用草稿，切任务/工程应明确保留",
    );
    text(&ctx, &mut prototype, size, "draft");
    let output = frame(&ctx, &mut prototype, Vec::new(), size);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position(&shape.shape, "临时旁查 B"))
        .unwrap();
    let _ = frame(
        &ctx,
        &mut prototype,
        vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        size,
    );
    // 原生按下和释放之间可以重绘多帧，不能在这里忘记编辑器返回点。
    for _ in 0..8 {
        let _ = frame(&ctx, &mut prototype, Vec::new(), size);
    }
    let _ = frame(
        &ctx,
        &mut prototype,
        vec![Event::PointerButton {
            pos: point,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        size,
    );
    assert!(prototype.drawer);
    escape(&ctx, &mut prototype, size);
    assert!(!prototype.drawer);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(draft_id));
    text(&ctx, &mut prototype, size, "x");
    let _ = frame(&ctx, &mut prototype, Vec::new(), size);
    text(&ctx, &mut prototype, size, "y");
    assert_eq!(prototype.drafts[0], "draftxy");
}

#[test]
fn temporary_reader_does_not_restore_a_draft_from_an_earlier_pointer_gesture() {
    let ctx = egui::Context::default();
    let mut prototype = Prototype::default();
    let size = [1024.0, 640.0];
    let draft_id = egui::Id::new(("eds11_draft", 0));
    click(&ctx, &mut prototype, size, "J2 写作旁查");
    click(
        &ctx,
        &mut prototype,
        size,
        "在此输入未应用草稿，切任务/工程应明确保留",
    );
    text(&ctx, &mut prototype, size, "draft");
    let output = frame(&ctx, &mut prototype, Vec::new(), size);
    let position = |label| {
        output
            .shapes
            .iter()
            .find_map(|shape| text_position(&shape.shape, label))
            .unwrap()
    };
    let other = position("固定 B");
    let drawer = position("临时旁查 B");
    click_at(&ctx, &mut prototype, size, other);
    assert_ne!(ctx.memory(|memory| memory.focused()), Some(draft_id));
    // 没有空闲帧的第二个点击，也不能复用之前那个手势的源码焦点。
    let _ = frame(
        &ctx,
        &mut prototype,
        vec![
            Event::PointerMoved(drawer),
            Event::PointerButton {
                pos: drawer,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        size,
    );
    for _ in 0..8 {
        let _ = frame(&ctx, &mut prototype, Vec::new(), size);
    }
    let _ = frame(
        &ctx,
        &mut prototype,
        vec![Event::PointerButton {
            pos: drawer,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        size,
    );
    assert!(prototype.drawer);
    escape(&ctx, &mut prototype, size);
    assert_ne!(ctx.memory(|memory| memory.focused()), Some(draft_id));
    assert_eq!(prototype.drafts[0], "draft");
}

#[test]
fn world_placement_cancel_apply_undo_and_guards_use_egui_events() {
    let ctx = egui::Context::default();
    let mut prototype = Prototype::default();
    let size = [1024.0, 640.0];

    click(&ctx, &mut prototype, size, "选入口 P");
    assert_eq!(prototype.selected, "P");
    drag_slider_left_of_label(&ctx, &mut prototype, size, "入口位置预览", -100.0, -35.0);
    let candidate = prototype.preview_position;
    assert_ne!(candidate, 30.0, "egui 拖动应改变候选位置");
    assert_eq!(prototype.placement_position, 30.0);

    click(&ctx, &mut prototype, size, "取消预览 / Escape");
    assert_eq!(prototype.preview_position, 30.0);
    assert_eq!(prototype.placement_position, 30.0);

    drag_slider_left_of_label(&ctx, &mut prototype, size, "入口位置预览", -100.0, -35.0);
    let candidate = prototype.preview_position;
    click(&ctx, &mut prototype, size, "应用一次（真实 core）");
    assert_eq!(prototype.placement_position, candidate);
    assert_eq!(prototype.undo_position, Some(30.0));
    click(&ctx, &mut prototype, size, "撤销一次（真实 core）");
    assert_eq!(prototype.placement_position, 30.0);
    assert_eq!(prototype.preview_position, 30.0);
    assert!(prototype.undo_position.is_none());

    drag_slider_left_of_label(&ctx, &mut prototype, size, "入口位置预览", -100.0, -35.0);
    let candidate = prototype.preview_position;
    click(&ctx, &mut prototype, size, "只读");
    click(&ctx, &mut prototype, size, "应用一次（真实 core）");
    assert_eq!(prototype.placement_position, 30.0);
    assert_eq!(prototype.preview_position, candidate);
    assert!(prototype.undo_position.is_none());

    click(&ctx, &mut prototype, size, "只读");
    click(&ctx, &mut prototype, size, "旧基线");
    click(&ctx, &mut prototype, size, "应用一次（真实 core）");
    assert_eq!(prototype.placement_position, 30.0);
    assert_eq!(prototype.preview_position, candidate);
    assert!(prototype.undo_position.is_none());

    click(&ctx, &mut prototype, size, "旧基线");
    click(&ctx, &mut prototype, size, "锁层");
    click(&ctx, &mut prototype, size, "应用一次（真实 core）");
    assert_eq!(prototype.placement_position, 30.0);
    assert_eq!(prototype.preview_position, candidate);
    assert!(prototype.undo_position.is_none());
    let output = frame(&ctx, &mut prototype, Vec::new(), size);
    assert!(output
        .shapes
        .iter()
        .any(|shape| contains_text(&shape.shape, "磁盘写入 0")));
}

#[test]
fn writing_project_and_task_switches_preserve_scoped_draft_via_events() {
    let ctx = egui::Context::default();
    let mut prototype = Prototype::default();
    let size = [1024.0, 640.0];

    click(&ctx, &mut prototype, size, "J2 写作旁查");
    click(
        &ctx,
        &mut prototype,
        size,
        "在此输入未应用草稿，切任务/工程应明确保留",
    );
    text(&ctx, &mut prototype, size, "雾港草稿");
    assert_eq!(prototype.drafts[0], "雾港草稿");

    click(&ctx, &mut prototype, size, "样例作品 1");
    click_last(&ctx, &mut prototype, size, "样例作品 2");
    assert_eq!(prototype.project, 0);
    assert_eq!(prototype.pending_project, Some(1));
    click(&ctx, &mut prototype, size, "取消切换");
    assert_eq!(prototype.project, 0);
    assert_eq!(prototype.drafts[0], "雾港草稿");

    click(&ctx, &mut prototype, size, "样例作品 1");
    click_last(&ctx, &mut prototype, size, "样例作品 2");
    click(&ctx, &mut prototype, size, "保留并切作品");
    assert_eq!(prototype.project, 1);
    assert_eq!(prototype.drafts[0], "雾港草稿");
    assert!(prototype.drafts[1].is_empty());

    click(&ctx, &mut prototype, size, "样例作品 2");
    click_last(&ctx, &mut prototype, size, "样例作品 1");
    assert_eq!(prototype.project, 0);
    click(&ctx, &mut prototype, size, "J3 演练");
    assert!(prototype.pending_task == Some(Task::Run));
    click(&ctx, &mut prototype, size, "取消切换");
    assert!(prototype.task == Task::Writing);
    assert_eq!(prototype.drafts[0], "雾港草稿");

    click(&ctx, &mut prototype, size, "J3 演练");
    click(&ctx, &mut prototype, size, "保留并切任务");
    assert!(prototype.task == Task::Run);
    assert_eq!(prototype.drafts[0], "雾港草稿");
}

#[test]
fn run_and_review_paths_keep_preview_runtime_and_comparison_separate() {
    let ctx = egui::Context::default();
    let mut prototype = Prototype::default();
    let size = [1024.0, 640.0];

    click(&ctx, &mut prototype, size, "J3 演练");
    assert!(prototype.task == Task::Run);
    let output = frame(&ctx, &mut prototype, Vec::new(), size);
    assert!(output.shapes.iter().any(|shape| {
        contains_text(&shape.shape, "worldline-core 真正编译样例：0 条诊断")
    }));
    click(&ctx, &mut prototype, size, "临时预览 E");
    assert!(prototype.preview_event);
    assert!(!prototype.runtime_event);
    click(&ctx, &mut prototype, size, "显式运行 E（真实 runtime）");
    assert!(prototype.preview_event && prototype.runtime_event);

    click(&ctx, &mut prototype, size, "J4 审阅");
    assert!(prototype.task == Task::Review);
    let before = (
        prototype.preview_event,
        prototype.runtime_event,
        prototype.drafts.clone(),
    );
    click(&ctx, &mut prototype, size, "重新比较（不采纳）");
    assert_eq!(
        before,
        (
            prototype.preview_event,
            prototype.runtime_event,
            prototype.drafts.clone()
        )
    );
    assert!(prototype.notice.contains("真实 core 三方比较"));
    let comparison = prototype.demos[0].review.as_ref().unwrap();
    assert_eq!(comparison.conflicts.len(), 1);
    assert!(!comparison.can_apply());
    assert_eq!(prototype.demos[0].applied_commands, 0);
    let run = prototype.demos[0].runtime.as_ref().unwrap();
    assert!(run.ended);
    assert_eq!(run.start_visits, 1);
    assert!(run.output.contains("雾港的晨钟再次响了。"));
    let output = frame(&ctx, &mut prototype, Vec::new(), size);
    for label in [
        "base：",
        "current：",
        "proposal：",
        "雾港的晨钟响了。",
        "雾港的晨钟再次响了。",
        "雾港钟声消失了。",
        "磁盘写入 0",
    ] {
        assert!(output
            .shapes
            .iter()
            .any(|shape| contains_text(&shape.shape, label)));
    }
}

#[test]
fn logical_viewport_survives_1024_and_700_points_at_scaled_dpi_and_zoom() {
    let ctx = egui::Context::default();
    let mut prototype = Prototype::default();
    ctx.set_zoom_factor(1.25);
    let output = frame_with_native_scale(&ctx, &mut prototype, [1024.0, 640.0], 1.5);
    assert!((output.pixels_per_point - 1.875).abs() < 0.01);
    assert!(output.shapes.iter().any(|shape| {
        contains_text(&shape.shape, "世界资料 / 地图入口（内存 core 样例）")
    }));

    click(&ctx, &mut prototype, [1024.0, 640.0], "窄窗区域");
    let narrow = frame_with_native_scale(&ctx, &mut prototype, [700.0, 640.0], 1.5);
    assert!(narrow.shapes.iter().any(|shape| {
        contains_text(&shape.shape, "世界资料 / 地图入口（内存 core 样例）")
    }));
    assert!(narrow
        .shapes
        .iter()
        .any(|shape| contains_text(&shape.shape, "检查器标签")));
    click(&ctx, &mut prototype, [700.0, 640.0], "检查器标签");
    let with_inspector = frame_with_native_scale(&ctx, &mut prototype, [700.0, 640.0], 1.5);
    assert!(with_inspector
        .shapes
        .iter()
        .any(|shape| contains_text(&shape.shape, "跟随选择")));
}

#[test]
fn placement_preview_cancel_apply_and_undo_are_local() {
    let mut prototype = Prototype {
        preview_position: 60.0,
        ..Default::default()
    };
    assert_eq!(prototype.placement_position, 30.0);
    prototype.cancel_preview();
    assert_eq!(prototype.preview_position, 30.0);
    prototype.preview_position = 60.0;
    prototype.apply_preview();
    assert_eq!(prototype.placement_position, 60.0);
    prototype.undo_preview();
    assert_eq!(prototype.placement_position, 30.0);
    assert_eq!(prototype.preview_position, 30.0);
}

#[test]
fn read_only_stale_and_locked_reject_apply_without_losing_candidate() {
    for blocked in 0..3 {
        let mut prototype = Prototype {
            preview_position: 60.0,
            read_only: blocked == 0,
            stale: blocked == 1,
            locked_layer: blocked == 2,
            ..Default::default()
        };
        prototype.apply_preview();
        assert_eq!(prototype.placement_position, 30.0);
        assert_eq!(prototype.preview_position, 60.0);
        assert!(prototype.undo_position.is_none());
    }
}

#[test]
fn switching_project_keeps_draft_bound_to_original_project() {
    let mut prototype = Prototype {
        task: Task::Writing,
        ..Default::default()
    };
    prototype.drafts[0] = "雾港草稿".into();
    prototype.choose_project(1);
    assert_eq!(prototype.project, 0);
    assert_eq!(prototype.pending_project, Some(1));
    prototype.project = prototype.pending_project.take().unwrap();
    assert!(prototype.drafts[1].is_empty());
    assert_eq!(prototype.drafts[0], "雾港草稿");
}

#[test]
fn isolated_prototype_installs_its_own_resolved_default_theme() {
    for system in [None, Some(egui::Theme::Light), Some(egui::Theme::Dark)] {
        let ctx = egui::Context::default();
        ctx.set_visuals(egui::Visuals::dark());
        let mut prototype = Prototype::default();
        let before = prototype.demos[0].project.content_baseline();
        let output = ctx.run(
            RawInput {
                system_theme: system,
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 760.0))),
                ..Default::default()
            },
            |ctx| prototype.draw(ctx),
        );
        let expected =
            crate::theme::resolve(&crate::theme::AppearancePreferences::default(), system);
        assert_eq!(crate::theme::resolved(&ctx), expected);
        assert_eq!(ctx.style().visuals.dark_mode, !expected.light);
        assert_eq!(ctx.style().visuals.panel_fill, expected.colors.panel);
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Rect(rect) if rect.fill == expected.colors.panel)));
        assert_eq!(prototype.demos[0].project.content_baseline(), before);
    }
}
