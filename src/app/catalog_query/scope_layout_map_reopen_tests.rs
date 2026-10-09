//! 原生 D04 的 Scene 状态行/面板切换/hover 子菜单/重复重开路径。
use super::*;

#[test]
fn full_app_scene_map_reopens_after_panel_collapse_hover_and_escape_then_accepts_input() {
    let (ctx, mut app) = ready_with_scene(false, true);
    let baseline = app.project.content_baseline();
    let sources = app.project.sources();
    let documents: Vec<_> = app
        .project
        .authoring_documents
        .iter()
        .map(|(path, document)| (path.clone(), document.bytes().to_vec()))
        .collect();
    let snapshot = app.catalog_workbench.snapshot.clone().unwrap();
    click(&ctx, &mut app, "地图操作");
    let output = settle(&ctx, &mut app);
    visible(&output, "矢量显示");
    click(&ctx, &mut app, "显示地图面板");
    settle(&ctx, &mut app);
    assert_eq!(
        ctx.data(|data| data.get_temp::<bool>(egui::Id::new("map-inspector-visible"))),
        Some(true)
    );
    click(&ctx, &mut app, "地图操作");
    let output = settle(&ctx, &mut app);
    let measure = visible(&output, "测距与校准");
    frame(&ctx, &mut app, vec![Event::PointerMoved(measure)]);
    let output = settle(&ctx, &mut app);
    visible(&output, "尺子 · 只读");
    assert!(menu_rects(&ctx).len() >= 2, "必须真实打开测距子菜单");
    let root_layer = menu_rects(&ctx)[0].0;
    let collapse = visible(&output, "收起地图面板");
    assert_eq!(ctx.layer_id_at(collapse), Some(root_layer));
    press(&ctx, &mut app, collapse);
    escape_menu(&ctx, &mut app);
    assert_eq!(
        ctx.data(|data| data.get_temp::<bool>(egui::Id::new("map-inspector-visible"))),
        Some(false)
    );
    clip(&ctx, "catalog-scope-map-clip");
    for _ in 0..2 {
        click(&ctx, &mut app, "地图操作");
        let output = settle(&ctx, &mut app);
        assert!(
            egui::Popup::is_any_open(&ctx),
            "同一个地图操作按钮必须能重复重开"
        );
        for label in ["显示地图面板", "编辑展示", "测距与校准", "矢量显示"] {
            visible(&output, label);
        }
        escape_menu(&ctx, &mut app);
        clip(&ctx, "catalog-scope-map-clip");
    }
    calibration_input_roundtrip(&ctx, &mut app);
    assert_eq!(app.tab, Tab::Map);
    assert!(app.pending.is_none() && app.draft_action.is_none() && !app.allow_close);
    assert!(std::sync::Arc::ptr_eq(
        app.catalog_workbench.snapshot.as_ref().unwrap(),
        &snapshot
    ));
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.sources(), sources);
    assert_eq!(
        app.project
            .authoring_documents
            .iter()
            .map(|(path, document)| (path.clone(), document.bytes().to_vec()))
            .collect::<Vec<_>>(),
        documents
    );
    assert!(app.history.is_empty());
}

// 合成同一帧 opener release + Esc；随后既有精确值断言证明原校准草稿仍在。
pub(super) fn open_menu_with_escape(ctx: &egui::Context, app: &mut WorldeditApp) {
    let pos = visible(&settle(ctx, app), "地图操作");
    frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
            Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
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
    assert!(
        !egui::Popup::is_any_open(ctx),
        "开菜单同帧的 Esc 必须只关闭菜单"
    );
}

pub(super) fn choose_calibration(ctx: &egui::Context, app: &mut WorldeditApp) {
    for label in ["编辑展示", "测距与校准"] {
        let output = settle(ctx, app);
        let menus = menu_rects(ctx);
        let root = menus.first().expect("真实地图操作菜单").0;
        // Scene 菜单重新排版后，先真实移回根层上部，关闭可能遮住下一个工具的 hover 子层。
        let anchor = point(&output, "显示地图目录")
            .or_else(|| point(&output, "收起地图目录"))
            .unwrap();
        assert_eq!(ctx.layer_id_at(anchor), Some(root));
        frame(ctx, app, vec![Event::PointerMoved(anchor)]);
        let output = settle(ctx, app);
        let pos = visible(&output, label);
        eprintln!(
            "map-calibration before {label}: edit={}; point={pos:?}; hit={:?}; menus={:?}",
            app.map_canvas.is_edit_mode(),
            ctx.layer_id_at(pos),
            menu_rects(ctx)
        );
        assert_eq!(
            ctx.layer_id_at(pos),
            Some(root),
            "工具文字必须真正位于可点击根层"
        );
        press(ctx, app, pos);
        settle(ctx, app);
        eprintln!(
            "map-calibration after {label}: edit={}; popup={}; menus={:?}",
            app.map_canvas.is_edit_mode(),
            egui::Popup::is_any_open(ctx),
            menu_rects(ctx)
        );
        assert!(egui::Popup::is_any_open(ctx));
        assert!(
            app.map_canvas.is_edit_mode(),
            "必须真实进入编辑展示，不能只看见底层文字"
        );
    }
    let output = settle(ctx, app);
    if point(&output, "两点校准").is_none() {
        fn texts(shape: &egui::Shape, clip: Rect, values: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => values.push(format!(
                    "{:?}: rect={:?}; clip={clip:?}",
                    text.galley.job.text,
                    text.galley.rect.translate(text.pos.to_vec2())
                )),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        texts(shape, clip, values);
                    }
                }
                _ => {}
            }
        }
        let mut values = Vec::new();
        for shape in &output.shapes {
            texts(&shape.shape, shape.clip_rect, &mut values);
        }
        eprintln!(
            "map-calibration missing control, actual text:\n{}",
            values.join("\n")
        );
    }
    let pos = visible(&output, "两点校准");
    let menus = menu_rects(ctx);
    assert!(menus.len() >= 2);
    assert_eq!(
        ctx.layer_id_at(pos),
        Some(menus.last().unwrap().0),
        "校准动作必须在真实子菜单层"
    );
    press(ctx, app, pos);
}
