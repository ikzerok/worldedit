use super::*;
use egui::{Event, Key, Modifiers, RawInput, Shape};
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
const LONG: &str = "经过很长很长的中文道路抵达东岸并且保留完整的选择理由与来源证据";

fn setup() -> (egui::Context, WorldeditApp, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "worldedit-parallel-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("world.wl"), format!("let toll = 2\nevent start as \"渡口\"\n  choice \"{LONG}\" if toll > 0\n    -> end\n  choice \"同名分支\" if toll == 2\n    -> end\n  choice \"同名分支\"\n    -> end\nevent end as \"重逢\"\n  -> END\n")).unwrap();
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, Some(root.join("world.wl")));
    // App 初始化会安装主题；关闭动画必须在其后进行。
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.tab = Tab::Timeline;
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    (ctx, app, root)
}
fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
    width: f32,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 660.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            app.author_shortcuts(ctx);
            if app.tab == Tab::Edit {
                app.source_tab(ctx);
            } else {
                app.canvas_tab(ctx);
            }
            app.command_window(ctx);
        },
    )
}
fn settle(ctx: &egui::Context, app: &mut WorldeditApp) -> egui::FullOutput {
    // egui 新 Window 的首帧用于测量尺寸，未必产生可见文字，不能把它当最终绘制。
    for _ in 0..3 {
        frame(ctx, app, vec![], 1188.0);
    }
    frame(ctx, app, vec![], 1188.0)
}
fn text(shape: &Shape, out: &mut String) {
    match shape {
        Shape::Text(t) => {
            out.push_str(&t.galley.job.text);
            out.push('\n');
        }
        Shape::Vec(shapes) => {
            for shape in shapes {
                text(shape, out);
            }
        }
        _ => {}
    }
}
fn rendered(output: &egui::FullOutput) -> String {
    let mut out = String::new();
    for shape in &output.shapes {
        text(&shape.shape, &mut out);
    }
    out
}
fn text_point(shape: &Shape, needle: &str) -> Option<Pos2> {
    match shape {
        Shape::Text(text) if text.galley.job.text.contains(needle) => {
            Some(text.pos + text.galley.size() * 0.5)
        }
        Shape::Vec(shapes) => shapes.iter().find_map(|shape| text_point(shape, needle)),
        _ => None,
    }
}
fn click_text(ctx: &egui::Context, app: &mut WorldeditApp, needle: &str) {
    let output = frame(ctx, app, vec![], 1188.0);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_point(&shape.shape, needle))
        .unwrap_or_else(|| panic!("找不到 {needle}: {}", rendered(&output)));
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
            1188.0,
        );
    }
}
fn key(key: Key, modifiers: Modifiers) -> Vec<Event> {
    [true, false]
        .into_iter()
        .map(|pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        })
        .collect()
}
fn details(app: &WorldeditApp) -> Details {
    let graph = &app.snapshot.as_ref().unwrap().result.analysis.graph;
    let group = projected_groups(graph, true)
        .into_iter()
        .find(|group| {
            graph.nodes[group.from].name == "start" && graph.nodes[group.to].name == "end"
        })
        .unwrap();
    Details {
        version: app.version,
        workspace: app.project.root.clone(),
        timeline: true,
        edges: group
            .edges
            .iter()
            .map(|index| graph.edges[*index].clone())
            .collect(),
        heading: "start → end".into(),
        trigger: Id::new("test-trigger"),
        return_focus: None,
        first_focus: true,
        group,
    }
}

#[test]
fn grouping_preserves_each_original_edge_and_projects_scene_endpoints_only_for_timeline() {
    let (_ctx, app, root) = setup();
    let mut graph = app.snapshot.as_ref().unwrap().result.analysis.graph.clone();
    let original = serde_json::to_string(&graph).unwrap();
    let groups = projected_groups(&graph, true);
    assert_eq!(groups.len(), 1);
    // 每个 choice 的可选入口与其显式 -> 是两个独立 core 边，不能丢掉后者。
    assert_eq!(groups[0].edges, (0..6).collect::<Vec<_>>());
    assert_eq!(
        group_label(&groups[0], &graph),
        "6 条连接 · 选择 3 · 直达 3"
    );
    assert_eq!(
        graph.edges.iter().map(|edge| edge.line).collect::<Vec<_>>(),
        vec![3, 4, 5, 6, 7, 8]
    );
    assert_eq!(serde_json::to_string(&graph).unwrap(), original);
    let mut scene = graph.nodes[0].clone();
    scene.name = "start.branch".into();
    scene.is_event = false;
    let scene_index = graph.nodes.len();
    graph.ids.insert(scene.name.clone(), scene_index as u32);
    graph.nodes.push(scene);
    let mut scene_edge = graph.edges[0].clone();
    scene_edge.from = scene_index as u32;
    scene_edge.kind = EdgeKind::Drift;
    graph.edges.push(scene_edge);
    graph
        .edges
        .iter_mut()
        .filter(|edge| edge.kind == EdgeKind::Choice)
        .nth(1)
        .unwrap()
        .kind = EdgeKind::Divert;
    let timeline = projected_groups(&graph, true);
    assert_eq!(timeline[0].edges.len(), 7);
    assert_eq!(
        group_label(&timeline[0], &graph),
        "7 条连接 · 选择 2 · 直达 4 · 漂流 1"
    );
    assert_eq!(projected_groups(&graph, false).len(), 2);
    let mut enter = graph.edges[0].clone();
    enter.kind = EdgeKind::Enter;
    graph.edges.push(enter);
    let mut internal = graph.edges[0].clone();
    internal.to = scene_index as u32;
    graph.edges.push(internal);
    assert_eq!(projected_groups(&graph, true)[0].edges.len(), 7);
    assert_eq!(
        projected_groups(&graph, false)
            .iter()
            .map(|group| group.edges.len())
            .sum::<usize>(),
        9
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn grouped_badge_renders_once_at_supported_widths_and_zoom_then_details_show_full_labels() {
    let (ctx, mut app, root) = setup();
    for tab in [Tab::Timeline, Tab::Graph] {
        app.tab = tab;
        for zoom in [0.55, 1.0, 1.5] {
            app.zoom = zoom;
            for width in [1040.0, 1188.0, 1280.0] {
                let output = frame(&ctx, &mut app, vec![], width);
                let result = rendered(&output);
                assert_eq!(
                    result.matches("6 条连接 · 选择 3 · 直达 3").count(),
                    1,
                    "{result}"
                );
                assert!(!result.contains("同名分支"), "平行标签不能继续叠画");
            }
        }
    }
    app.tab = Tab::Timeline;
    app.zoom = 1.0;
    click_text(&ctx, &mut app, "6 条连接");
    let output = frame(&ctx, &mut app, vec![], 1040.0);
    let result = rendered(&output);
    assert!(result.contains(LONG), "{result}");
    assert!(result.contains("条件："));
    let saved = ctx
        .data(|data| data.get_temp::<Details>(state_id()))
        .unwrap();
    assert_eq!(saved.edges.len(), 6);
    assert_eq!(
        saved
            .edges
            .iter()
            .filter(|edge| edge.label.as_deref() == Some("同名分支"))
            .count(),
        2
    );
    // 详情窗口有界，后面的原始边应通过真实滚动变得可见；不能要求六行同时装下。
    let pointer = output
        .shapes
        .iter()
        .find_map(|shape| text_point(&shape.shape, "定位第 1 条来源"))
        .unwrap();
    let mut seen = std::collections::BTreeSet::new();
    let mut output = output;
    for _ in 0..24 {
        let result = rendered(&output);
        for index in 1..=6 {
            if result.contains(&format!("定位第 {index} 条来源")) {
                seen.insert(index);
            }
        }
        if seen.len() == 6 {
            break;
        }
        output = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pointer),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -120.0),
                    modifiers: Modifiers::NONE,
                },
            ],
            1040.0,
        );
    }
    assert_eq!(seen, (1..=6).collect());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn keyboard_can_close_reopen_and_navigate_each_original_source_then_back_without_writes() {
    let (ctx, mut app, root) = setup();
    let before = (
        app.project.sources(),
        app.project.is_dirty(),
        app.history.len(),
    );
    click_text(&ctx, &mut app, "6 条连接");
    frame(&ctx, &mut app, key(Key::Escape, Modifiers::NONE), 1188.0);
    assert!(ctx
        .data(|data| data.get_temp::<Details>(state_id()))
        .is_none());
    // Esc 把焦点返回原数量入口，Enter 重新展开。
    frame(&ctx, &mut app, key(Key::Enter, Modifiers::NONE), 1188.0);
    assert!(ctx
        .data(|data| data.get_temp::<Details>(state_id()))
        .is_some());
    let saved = ctx
        .data(|data| data.get_temp::<Details>(state_id()))
        .unwrap();
    for index in 0..saved.edges.len() {
        assert!(app.navigate_graph_edge(&saved, index));
        assert_eq!(app.jump, Some((saved.edges[index].line, 1)));
        assert_eq!(app.tab, Tab::Edit);
        app.author_back(&ctx);
        assert_eq!(app.tab, Tab::Timeline);
        assert!(ctx
            .data(|data| data.get_temp::<Details>(state_id()))
            .is_some());
    }
    assert_eq!(
        (
            app.project.sources(),
            app.project.is_dirty(),
            app.history.len()
        ),
        before
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stale_deleted_replaced_or_externally_changed_edge_never_uses_old_source_line() {
    let (ctx, mut app, root) = setup();
    let saved = details(&app);
    app.version += 1;
    assert!(!app.navigate_graph_edge(&saved, 0));
    app.version -= 1;
    app.snapshot.as_mut().unwrap().result.analysis.graph.edges[0].label = Some("different".into());
    assert!(!app.navigate_graph_edge(&saved, 0));
    app.recompile();
    let saved = details(&app);
    std::fs::write(root.join("world.wl"), "event other\n  -> END\n").unwrap();
    assert!(!app.navigate_graph_edge(&saved, 0));
    assert_eq!(app.tab, Tab::Timeline);
    assert!(app.personal.history.is_empty());
    ctx.data_mut(|data| data.insert_temp(state_id(), saved));
    app.version += 1;
    let output = settle(&ctx, &mut app);
    let visible = rendered(&output);
    assert!(visible.contains("旧来源定位已停用"), "{visible}");
    // 稳定绘制也不能改变之前的导航拒绝或消费历史。
    assert_eq!(app.tab, Tab::Timeline);
    assert!(app.personal.history.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn single_edges_retain_readable_label_and_palette_hides_lower_edge_details() {
    let (ctx, mut app, root) = setup();
    let mut graph = app.snapshot.as_ref().unwrap().result.analysis.graph.clone();
    graph.edges.truncate(1);
    let groups = projected_groups(&graph, true);
    let label = group_label(&groups[0], &graph);
    assert!(label.starts_with("选择 · "));
    assert!(label.contains("经过"));
    let saved = details(&app);
    ctx.data_mut(|data| data.insert_temp(state_id(), saved));
    app.open_commands(&ctx, true);
    assert!(app.command_palette.open && app.command_palette.commands_only);
    let output = settle(&ctx, &mut app);
    let text = rendered(&output);
    assert!(text.contains("任务命令"), "{text}");
    assert!(app.command_palette.open && app.command_palette.commands_only);
    assert!(!text.contains("连接详情 · 原始分支"));
    assert!(!text.contains("拖动卡片调整位置,右侧圆点用于连线"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn keyboard_opened_details_suppress_existing_background_tooltip_until_closed() {
    const HOVER: &str = "拖动卡片调整位置,右侧圆点用于连线";
    let (ctx, mut app, root) = setup();
    ctx.style_mut(|style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.tooltip_grace_time = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
    let before = app.project.sources();
    // 先关闭一次真实打开的详情，Esc 把键盘焦点留在可重新展开的数量按钮。
    click_text(&ctx, &mut app, "6 条连接");
    let trigger = ctx
        .data(|data| data.get_temp::<Details>(state_id()))
        .unwrap()
        .trigger;
    frame(&ctx, &mut app, key(Key::Escape, Modifiers::NONE), 1188.0);
    let output = settle(&ctx, &mut app);
    let pointer = output
        .shapes
        .iter()
        .find_map(|shape| text_point(&shape.shape, "重逢"))
        .unwrap();
    let mut hovered = false;
    for _ in 0..8 {
        let output = frame(&ctx, &mut app, vec![Event::PointerMoved(pointer)], 1188.0);
        hovered |= rendered(&output).contains(HOVER);
    }
    assert!(hovered, "应先真实显示背景节点提示，避免空洞的遮挡断言");
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(trigger));
    let mut events = vec![Event::PointerMoved(pointer)];
    events.extend(key(Key::Enter, Modifiers::NONE));
    let output = frame(&ctx, &mut app, events, 1188.0);
    assert!(details_visible(&app, &ctx));
    assert!(
        !rendered(&output).contains(HOVER),
        "打开帧也不能画底层 tooltip"
    );
    let mut full_context_visible = false;
    for index in 0..6 {
        let mut events = vec![Event::PointerMoved(pointer)];
        if index == 3 {
            events.extend(key(Key::Tab, Modifiers::NONE));
        }
        let output = frame(&ctx, &mut app, events, 1188.0);
        let visible = rendered(&output);
        assert!(!visible.contains(HOVER), "{visible}");
        full_context_visible |= visible.contains(LONG)
            && visible.contains("条件：")
            && visible.contains("定位第 1 条来源");
    }
    assert!(full_context_visible, "详情正文、完整选择上下文和来源应可读");
    let mut events = vec![Event::PointerMoved(pointer)];
    events.extend(key(Key::Escape, Modifiers::NONE));
    frame(&ctx, &mut app, events, 1188.0);
    assert!(!details_visible(&app, &ctx));
    let mut restored = false;
    for _ in 0..8 {
        let output = frame(&ctx, &mut app, vec![Event::PointerMoved(pointer)], 1188.0);
        restored |= rendered(&output).contains(HOVER);
    }
    assert!(restored, "详情关闭后应恢复正常节点悬停");
    assert_eq!(app.project.sources(), before);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn tooltip_suppression_tracks_visible_detail_workspace_and_view_even_when_stale() {
    let (ctx, mut app, root) = setup();
    ctx.data_mut(|data| data.insert_temp(state_id(), details(&app)));
    assert!(details_visible(&app, &ctx));
    app.version += 1;
    assert!(details_visible(&app, &ctx), "过期提示窗口同样在前景");
    app.tab = Tab::Graph;
    assert!(!details_visible(&app, &ctx));
    app.tab = Tab::Edit;
    assert!(!details_visible(&app, &ctx));
    app.tab = Tab::Timeline;
    app.project.root = root.join("another-workspace");
    assert!(!details_visible(&app, &ctx));
    let _ = std::fs::remove_dir_all(root);
}
