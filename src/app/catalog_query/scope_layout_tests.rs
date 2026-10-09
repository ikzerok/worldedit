//! 完整 App chrome、400×300 逻辑点/200% 下的真实菜单点击和材料滚动。
//! shape 必须完整落在真实 clip 内；不把直接 map_tab 绘制当完整窗口验收。
use super::*;
use crate::app::Tab;
use egui::{vec2, Event, PointerButton, Pos2, Rect, Vec2};
use worldline_core::queries::CatalogQueryFilter;

#[path = "scope_layout_map_reopen_tests.rs"]
mod map_reopen;
#[path = "scope_layout_network_tests.rs"]
mod network;
#[path = "scope_layout_picker_tests.rs"]
mod picker;

const SIZE: Vec2 = vec2(400.0, 300.0);

fn frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
    let mut input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)),
        time: Some(ctx.cumulative_frame_nr() as f64 / 60.0),
        events,
        ..Default::default()
    };
    input
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .native_pixels_per_point = Some(1.0);
    eframe::App::raw_input_hook(app, ctx, &mut input);
    let output = ctx.run(input, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest())
    });
    assert_eq!(ctx.screen_rect().size(), SIZE);
    assert_eq!(output.pixels_per_point, 2.0);
    output
}
fn settle(ctx: &egui::Context, app: &mut WorldeditApp) -> egui::FullOutput {
    frame(ctx, app, vec![]);
    frame(ctx, app, vec![]);
    frame(ctx, app, vec![])
}
fn text_rects(shape: &egui::Shape, label: &str, clip: Rect, found: &mut Vec<(Rect, Rect)>) {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text.contains(label) => {
            found.push((text.galley.rect.translate(text.pos.to_vec2()), clip));
        }
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                text_rects(shape, label, clip, found);
            }
        }
        _ => {}
    }
}
fn point(output: &egui::FullOutput, label: &str) -> Option<Pos2> {
    let mut found = Vec::new();
    for shape in &output.shapes {
        text_rects(&shape.shape, label, shape.clip_rect, &mut found);
    }
    let screen = Rect::from_min_size(Pos2::ZERO, SIZE);
    found
        .into_iter()
        .find(|(rect, clip)| screen.contains_rect(*rect) && clip.contains_rect(*rect))
        .map(|(rect, _)| rect.center())
}
fn visible(output: &egui::FullOutput, label: &str) -> Pos2 {
    point(output, label).unwrap_or_else(|| {
        let mut geometry = Vec::new();
        for shape in &output.shapes {
            text_rects(&shape.shape, label, shape.clip_rect, &mut geometry);
        }
        panic!("完整窗口中 {label:?} 必须完全位于屏幕和其真实 clip 内：{geometry:?}");
    })
}
fn exact_visible(output: &egui::FullOutput, label: &str) {
    fn visit(shape: &egui::Shape, label: &str, clip: Rect) -> bool {
        match shape {
            egui::Shape::Text(text) if text.galley.job.text == label => {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                clip.contains_rect(rect)
                    && Rect::from_min_size(Pos2::ZERO, SIZE).contains_rect(rect)
            }
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| visit(shape, label, clip)),
            _ => false,
        }
    }
    assert!(
        output
            .shapes
            .iter()
            .any(|shape| visit(&shape.shape, label, shape.clip_rect)),
        "实际输入值 {label:?} 必须完整可见，不能以含此字串的placeholder替代"
    );
}
fn press(ctx: &egui::Context, app: &mut WorldeditApp, pos: Pos2) {
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}
fn click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let output = settle(ctx, app);
    press(ctx, app, visible(&output, label));
}
fn click_material(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    settle(ctx, app);
    let pointer = clip(ctx, "catalog-scope-material-clip").center();
    let position = scroll_find(ctx, app, label, pointer);
    press(ctx, app, position);
}
fn operations_page(ctx: &egui::Context, app: &mut WorldeditApp) {
    let output = settle(ctx, app);
    visible(&output, "返回关系图");
    visible(&output, "关系操作");
    clip(ctx, "catalog-scope-material-clip");
    assert_eq!(
        ctx.data(|data| data.get_temp::<u8>(egui::Id::new("catalog-scope-network-pane"))),
        Some(3)
    );
    assert!(!egui::Popup::is_any_open(ctx), "操作页自身不是 popup");
}
fn foreground_rects(ctx: &egui::Context) -> Vec<(egui::LayerId, Rect)> {
    ctx.memory(|memory| {
        memory
            .areas()
            .visible_layer_ids()
            .into_iter()
            .filter(|layer| layer.order == egui::Order::Foreground)
            .filter_map(|layer| memory.area_rect(layer.id).map(|rect| (layer, rect)))
            .collect()
    })
}
fn menu_rects(ctx: &egui::Context) -> Vec<(egui::LayerId, Rect)> {
    let visible = foreground_rects(ctx);
    let mut current = visible
        .iter()
        .find(|(layer, _)| egui::Popup::is_id_open(ctx, layer.id))
        .copied();
    let mut result = Vec::new();
    // Popup 的 memory root → egui 真实 MenuState.open_item 子树，排除窗口 resize Area。
    while let Some((layer, rect)) = current {
        result.push((layer, rect));
        let child = ctx.data(|data| {
            data.get_temp::<egui::containers::menu::MenuState>(
                layer.id.with(egui::containers::menu::MenuState::ID),
            )
            .and_then(|state| state.open_item)
        });
        current = child.and_then(|id| visible.iter().find(|(layer, _)| layer.id == id).copied());
    }
    result
}
#[track_caller]
fn dismiss_outside(ctx: &egui::Context, app: &mut WorldeditApp) {
    let output = settle(ctx, app);
    let before = menu_rects(ctx);
    assert!(!before.is_empty() && egui::Popup::is_any_open(ctx));
    assert!(app.pending.is_none() && app.draft_action.is_none() && !app.allow_close);
    let tab = app.tab;
    let project = (
        app.project.root.clone(),
        app.project.content_baseline(),
        app.project.is_dirty(),
        app.history.len(),
    );
    // chrome.rs 的 worldedit 是普通 Label，无按钮动作；仍核真实 clip、菜单边界和命中层。
    let pos = visible(&output, "worldedit");
    assert!(
        before
            .iter()
            .all(|(_, rect)| !rect.expand(1.0).contains(pos))
            && ctx
                .layer_id_at(pos)
                .is_none_or(|layer| layer.order != egui::Order::Foreground),
        "惰性标签必须真实露在根/子菜单外：point={pos:?}; menus={before:?}"
    );
    let old_corner = Pos2::new(SIZE.x - 1.0, SIZE.y - 1.0);
    eprintln!("popup-dismiss: old_corner_layer={:?}; chosen={pos:?}; menus={before:?}; depth={}; direction={:?}",
        ctx.layer_id_at(old_corner), app.network_state.filters.depth, app.network_state.filters.direction);
    // 保留一次真实按下/松开。不得用第二次点击、直接close或Esc补救外部关闭。
    press(ctx, app, pos);
    settle(ctx, app);
    assert_eq!(app.tab, tab, "关闭菜单不得切换工作视图");
    assert!(
        app.pending.is_none() && app.draft_action.is_none() && !app.allow_close,
        "关闭菜单不得请求关闭工程或触发草稿保护"
    );
    assert_eq!(
        (
            app.project.root.clone(),
            app.project.content_baseline(),
            app.project.is_dirty(),
            app.history.len()
        ),
        project
    );
    let after: Vec<_> = foreground_rects(ctx)
        .into_iter()
        .filter(|(layer, _)| before.iter().any(|(old, _)| old == layer))
        .collect();
    assert!(
        !egui::Popup::is_any_open(ctx) && after.is_empty(),
        "一次实测外部点击必须关闭根/子菜单：point={pos:?}; before={before:?}; after={after:?}"
    );
}
fn escape_menu(ctx: &egui::Context, app: &mut WorldeditApp) {
    frame(
        ctx,
        app,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    frame(
        ctx,
        app,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    settle(ctx, app);
    assert!(!egui::Popup::is_any_open(ctx), "Esc 必须关闭上层菜单");
}
fn clip(ctx: &egui::Context, name: &str) -> Rect {
    let rect = ctx
        .data(|data| data.get_temp::<Rect>(egui::Id::new(name)))
        .expect("真实正文 clip");
    assert!(
        Rect::from_min_size(Pos2::ZERO, SIZE).contains_rect(rect),
        "{name}: {rect:?}"
    );
    assert!(
        rect.width() >= 300.0 && rect.height() >= 48.0,
        "完整 chrome 后必须留下可用图体/材料正文，不能只剩控件上下沿：{name}: {rect:?}"
    );
    rect
}
fn scroll_find(ctx: &egui::Context, app: &mut WorldeditApp, label: &str, pointer: Pos2) -> Pos2 {
    for _ in 0..40 {
        let output = settle(ctx, app);
        if let Some(pos) = point(&output, label) {
            return pos;
        }
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pointer),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -20.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    visible(&settle(ctx, app), label)
}
fn ready(network: bool) -> (egui::Context, WorldeditApp) {
    ready_with_scene(network, false)
}
fn ready_with_scene(network: bool, with_scene: bool) -> (egui::Context, WorldeditApp) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "scope-full-app-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut project = worldline_core::project::Project::new(&root);
    project.set_text(&project.entry.clone(), "event start\n  -> END\nentity harbor kind place as \"港口\"\nentity neighbor kind place as \"邻港\"\nentity beyond kind place as \"山口\"\nrelation_type route as \"道路\"\nrelation_def first type route from entity harbor to entity neighbor\nrelation_def second type route from entity neighbor to entity beyond\n".into()).unwrap();
    project.create_authoring_document(&root.join(".world/project.json"), serde_json::to_vec(&serde_json::json!({
        "schema_version":1,"project_id":"scope-layout","language_version":"1.10","entry":"world.wl",
        "required_features":["content.entities.v1","content.relations.v1"],
        "maps":{"atlas":".world/maps/atlas.json"}
    })).unwrap()).unwrap();
    let mut map = serde_json::json!({
        "schema_version":1,"id":"atlas","title":"港口地图","canvas":{"width":1000,"height":800,"unit":"normalized"},
        "layer_order":["places"],"layers":{"places":{"title":"地点","visible_default":true,"locked":false}},
        "placements":{"port":{"layer_id":"places","target_ref":{"kind":"entity","id":"harbor"},"geometry":{"kind":"point","position":[0.5,0.5]},"annotation":"","role":"location"}},"extensions":{}
    });
    if with_scene {
        map["required_features"] = serde_json::json!(["presentation.vector_scene.v1"]);
        map["scene"] =
            serde_json::to_value(worldline_core::vector_scene::MapScene::new(1000.0, 800.0))
                .unwrap();
    }
    project
        .create_authoring_document(
            &root.join(".world/maps/atlas.json"),
            serde_json::to_vec(&map).unwrap(),
        )
        .unwrap();
    let ctx = egui::Context::default();
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    app.project = project;
    app.active_file = app.project.entry.clone();
    app.saved_location = false;
    app.personal.pending_restore = false;
    app.personal.settings.style = crate::theme::StylePreset::Studio;
    app.personal.settings.ui_scale = 1.0;
    app.personal.settings.reduce_motion = true;
    app.recompile();
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .map_index
        .diagnostics
        .is_empty());
    app.tab = Tab::Map;
    app.map_selection = Some("atlas".into());
    // egui 0.32.3 在消费 pending zoom 时，将上一帧 screen_rect 按旧/新 zoom
    // 缩放，并覆盖本帧传入矩形。先通过完整 App 建立真实的 800×600/1×
    // 输入，避免新 Context 的默认 10000² 被缩成 5000²；这帧不作小窗证据。
    let mut initial = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE * 2.0)),
        ..Default::default()
    };
    initial
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .native_pixels_per_point = Some(1.0);
    eframe::App::raw_input_hook(&mut app, &ctx, &mut initial);
    let initialized = ctx.run(initial, |ctx| {
        eframe::App::update(&mut app, ctx, &mut eframe::Frame::_new_kittest());
    });
    assert_eq!(ctx.screen_rect().size(), SIZE * 2.0);
    assert_eq!(initialized.pixels_per_point, 1.0);
    app.personal.settings.ui_scale = 2.0;
    ctx.set_zoom_factor(2.0);
    settle(&ctx, &mut app);
    app.tab = Tab::Catalog;
    let mut state = WorkbenchState {
        query: CatalogQuery {
            filters: vec![CatalogQueryFilter::Name {
                values: vec!["harbor".into()],
                negate: false,
            }],
            ..Default::default()
        },
        ..Default::default()
    };
    let snapshot = app
        .project
        .catalog_scope_snapshot(&state.query, state.max_candidates)
        .unwrap();
    assert_eq!(snapshot.query().total(), 1);
    assert_eq!(snapshot.counts().matching_placements, 1);
    state.page = Some(snapshot.query().page(0, state.page_size).unwrap());
    state.snapshot = Some(std::sync::Arc::new(snapshot));
    state.snapshot_key = Some((app.version, app.map_revision));
    state.snapshot_query = Some(state.query.clone());
    state.snapshot_observation = Some(app.project.catalog_scope_observation_key());
    state.enter_scope(&mut app, &ctx, network);
    app.catalog_workbench = state;
    (ctx, app)
}
fn scope_menu(ctx: &egui::Context, app: &mut WorldeditApp) {
    click(ctx, app, "范围操作");
    let output = settle(ctx, app);
    for label in ["返回同一查询", "返回巡检位置", "刷新范围", "清除范围并恢复"]
    {
        visible(&output, label);
    }
}

#[test]
fn full_app_scope_map_200_percent_keeps_canvas_and_return_clear_menus_reachable() {
    let (ctx, mut app) = ready(false);
    let baseline = app.project.content_baseline();
    let snapshot = app.catalog_workbench.snapshot.clone().unwrap();
    let output = settle(&ctx, &mut app);
    for label in ["worldedit", "保存全部", "范围 · 1", "范围操作", "地图操作"] {
        visible(&output, label);
    }
    clip(&ctx, "catalog-scope-map-clip");
    click(&ctx, &mut app, "地图操作");
    let output = settle(&ctx, &mut app);
    for label in [
        "显示地图目录",
        "显示地图面板",
        "浏览",
        "编辑展示",
        "适配全图",
    ] {
        visible(&output, label);
    }
    press(&ctx, &mut app, visible(&output, "适配全图"));
    // 真实点击菜单外关闭，再执行返回和清除。
    dismiss_outside(&ctx, &mut app);
    scope_menu(&ctx, &mut app);
    click(&ctx, &mut app, "返回同一查询");
    assert_eq!(app.tab, Tab::Catalog);
    assert!(std::sync::Arc::ptr_eq(
        app.catalog_workbench.snapshot.as_ref().unwrap(),
        &snapshot
    ));
    assert_eq!(app.catalog_workbench.page.as_ref().unwrap().offset, 0);
    scope_menu(&ctx, &mut app);
    click(&ctx, &mut app, "返回巡检位置");
    assert_eq!(app.tab, Tab::Map);
    settle(&ctx, &mut app);
    clip(&ctx, "catalog-scope-map-clip");
    scope_menu(&ctx, &mut app);
    click(&ctx, &mut app, "清除范围并恢复");
    assert_eq!(app.tab, Tab::Catalog);
    assert!(!app.query_scope_active());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn full_app_scope_map_menu_retains_calibration_input_and_escape_only_closes_menu() {
    let (ctx, mut app) = ready(false);
    calibration_input_roundtrip(&ctx, &mut app);
}
fn calibration_input_roundtrip(ctx: &egui::Context, app: &mut WorldeditApp) {
    let baseline = app.project.content_baseline();
    click(ctx, app, "地图操作");
    map_reopen::choose_calibration(ctx, app);
    settle(ctx, app);
    assert!(egui::Popup::is_any_open(ctx));
    let pointer = ctx
        .data(|data| data.get_temp::<Rect>(egui::Id::new("catalog-scope-map-actions-clip")))
        .unwrap()
        .center();
    let field = scroll_find(ctx, app, "例如 12.5", pointer);
    press(ctx, app, field);
    frame(ctx, app, vec![Event::Text("12.5".into())]);
    assert!(
        egui::Popup::is_any_open(ctx),
        "点击/输入校准距离不能关闭菜单"
    );
    exact_visible(&settle(ctx, app), "12.5");
    escape_menu(ctx, app);
    map_reopen::open_menu_with_escape(ctx, app);
    click(ctx, app, "地图操作");
    let pointer = ctx
        .data(|data| data.get_temp::<Rect>(egui::Id::new("catalog-scope-map-actions-clip")))
        .unwrap()
        .center();
    scroll_find(ctx, app, "12.5", pointer);
    exact_visible(&settle(ctx, app), "12.5");
    let field = scroll_find(ctx, app, "例如 千米", pointer);
    press(ctx, app, field);
    frame(ctx, app, vec![Event::Text("千米".into())]);
    exact_visible(&settle(ctx, app), "千米");
    assert!(egui::Popup::is_any_open(ctx));
    dismiss_outside(ctx, app);
    click(ctx, app, "地图操作");
    let pointer = ctx
        .data(|data| data.get_temp::<Rect>(egui::Id::new("catalog-scope-map-actions-clip")))
        .unwrap()
        .center();
    scroll_find(ctx, app, "千米", pointer);
    exact_visible(&settle(ctx, app), "千米");
    let cancel = scroll_find(ctx, app, "取消校准", pointer);
    press(ctx, app, cancel);
    dismiss_outside(ctx, app);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}
