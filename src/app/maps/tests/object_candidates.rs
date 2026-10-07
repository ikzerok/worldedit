use super::*;
use egui::{Event, PointerButton, Pos2, RawInput, Rect, Vec2};
use worldline_core::catalog::TargetRef;
use worldline_core::navigation::AliasInfo;

fn catalog() -> Catalog {
    Catalog {
        objects: (0..45)
            .rev()
            .map(|index| CatalogObject {
                target: TargetRef::new("entity", &format!("object_{index:03}")),
                display: "同名".into(),
                file: "world.wl".into(),
                line: index + 1,
            })
            .chain(std::iter::once(CatalogObject {
                target: TargetRef::new("character", "object_044"),
                display: "同名".into(),
                file: "people.wl".into(),
                line: 1,
            }))
            .collect(),
        aliases: vec![AliasInfo {
            target: TargetRef::new("entity", "object_044"),
            name: "另名 ÄLIAS".into(),
            file: "aliases.wl".into(),
            line: 99,
        }],
        ..Default::default()
    }
}

#[test]
fn map_candidate_pages_reach_all_identities_and_aliases_without_local_truncation() {
    let catalog = catalog();
    let mut state = CandidateSearch::default();
    let mut targets = Vec::new();
    loop {
        state.refresh(&catalog, "同名", 1);
        let page = state.page.as_ref().unwrap().as_ref().unwrap();
        assert_eq!(page.total, 46);
        targets.extend(page.items.iter().map(|object| object.target.clone()));
        let Some(next) = page.next_offset else { break };
        state.options.offset = next;
    }
    assert_eq!(targets.len(), 46);
    assert!(targets.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(targets.contains(&TargetRef::new("entity", "object_044")));
    assert!(targets.contains(&TargetRef::new("character", "object_044")));
    for (query, total) in [
        ("Älias", 1),
        ("另名", 1),
        ("OBJECT_044", 2),
        ("character", 1),
        ("entity", 45),
    ] {
        state.refresh(&catalog, query, 1);
        let page = state.page.as_ref().unwrap().as_ref().unwrap();
        assert_eq!(page.total, total, "{query}");
        assert_eq!(page.offset, 0);
        if total == 1 && query != "character" {
            assert_eq!(page.items[0].target, TargetRef::new("entity", "object_044"));
            assert_eq!(page.items[0].file, "world.wl");
        }
    }
}

#[test]
fn map_candidate_cache_resets_after_edit_and_budget_errors_return_no_partial_page() {
    let mut catalog = catalog();
    let mut state = CandidateSearch::default();
    state.refresh(&catalog, "同名", 1);
    state.options.offset = 40;
    state.refresh(&catalog, "同名", 1);
    assert_eq!(state.page.as_ref().unwrap().as_ref().unwrap().offset, 40);
    catalog.objects.truncate(2);
    state.refresh(&catalog, "同名", 2);
    let page = state.page.as_ref().unwrap().as_ref().unwrap();
    assert_eq!((page.total, page.offset), (2, 0));
    state.options.max_candidates = 1;
    state.refresh(&catalog, "同名", 2);
    assert!(state
        .page
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap_err()
        .contains("未返回部分结果"));
    state.options.max_candidates = 2;
    state.refresh(&catalog, "同名", 2);
    assert_eq!(state.page.as_ref().unwrap().as_ref().unwrap().total, 2);
    state.refresh(&catalog, "absent", 2);
    let page = state.page.as_ref().unwrap().as_ref().unwrap();
    assert_eq!((page.total, page.offset), (0, 0));
    assert!(!page.truncated);
}

#[cfg(not(target_arch = "wasm32"))]
mod ui {
    use super::*;
    use crate::app::maps::CanvasMode;
    use crate::app::WorldeditApp;
    use worldline_core::project::Project;
    use worldline_core::vector_scene::{SceneGeometry, SceneNode};

    #[derive(Clone, Copy)]
    enum Panel {
        Reverse,
        Marker,
        Scene,
    }

    pub(super) fn fixture() -> (egui::Context, WorldeditApp) {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.0);
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = WorldeditApp::new(&creation, None);
        app.project = Project::new(&std::env::temp_dir().join(format!(
            "worldedit-map-candidate-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        )));
        let mut source = "event start\n  -> END\ncharacter object_044 as \"同名\"\n".to_string();
        for index in 0..45 {
            source.push_str(&format!(
                "entity object_{index:03} kind place as \"同名\"\n"
            ));
        }
        source.push_str("alias entity object_044 as \"另名\"\n");
        app.project
            .set_text(&app.project.entry.clone(), source)
            .unwrap();
        app.project.create_authoring_document(
            &app.project.root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.10","entry":"world.wl","required_features":["content.entities.v1"]}"#.to_vec(),
        ).unwrap();
        app.recompile();
        assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
        app.map_canvas.set_mode(CanvasMode::Edit);
        app.map_canvas.scene.inspector = Some(SceneNode::new(
            "node",
            "layer",
            SceneGeometry::Point {
                position: [0.5, 0.5],
            },
        ));
        (ctx, app)
    }

    fn frame(
        ctx: &egui::Context,
        app: &mut WorldeditApp,
        panel: Panel,
        events: Vec<Event>,
    ) -> egui::FullOutput {
        ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1500.0, 3000.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| match panel {
                    Panel::Reverse => app.map_search_panel(ui),
                    Panel::Marker => app.map_markers_panel(ui, None, None),
                    Panel::Scene => {
                        app.scene_inspector(ui);
                    }
                });
            },
        )
    }

    fn position(shape: &egui::epaint::Shape, label: &str) -> Option<Pos2> {
        match shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            egui::epaint::Shape::Vec(shapes) => {
                shapes.iter().find_map(|shape| position(shape, label))
            }
            _ => None,
        }
    }

    fn text(shape: &egui::epaint::Shape, output: &mut String) {
        match shape {
            egui::epaint::Shape::Text(value) => {
                output.push_str(value.galley.text());
                output.push('\n');
            }
            egui::epaint::Shape::Vec(shapes) => {
                for shape in shapes {
                    text(shape, output);
                }
            }
            _ => {}
        }
    }

    fn shown(ctx: &egui::Context, app: &mut WorldeditApp, panel: Panel) -> String {
        let output = frame(ctx, app, panel, Vec::new());
        let mut result = String::new();
        for shape in &output.shapes {
            text(&shape.shape, &mut result);
        }
        result
    }

    fn click(ctx: &egui::Context, app: &mut WorldeditApp, panel: Panel, label: &str) {
        let output = frame(ctx, app, panel, Vec::new());
        let point = output
            .shapes
            .iter()
            .find_map(|shape| position(&shape.shape, label))
            .unwrap_or_else(|| panic!("未显示 {label}: {}", shown(ctx, app, panel)));
        for pressed in [true, false] {
            frame(
                ctx,
                app,
                panel,
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
    fn reverse_lookup_shows_total_last_page_alias_and_recompile_reset_without_writes() {
        let (ctx, mut app) = fixture();
        app.map_search = "同名".into();
        let baseline = app.project.content_baseline();
        click(&ctx, &mut app, Panel::Reverse, "对象反查");
        let first = shown(&ctx, &mut app, Panel::Reverse);
        assert!(first.contains("46 个匹配对象 · 显示 1–20"), "{first}");
        click(&ctx, &mut app, Panel::Reverse, "下一页候选");
        click(&ctx, &mut app, Panel::Reverse, "下一页候选");
        let last = shown(&ctx, &mut app, Panel::Reverse);
        assert!(last.contains("显示 41–46"), "{last}");
        assert!(last.contains("entity:object_044"), "{last}");
        click(&ctx, &mut app, Panel::Reverse, "上一页候选");
        assert!(shown(&ctx, &mut app, Panel::Reverse).contains("显示 21–40"));
        app.recompile();
        assert!(shown(&ctx, &mut app, Panel::Reverse).contains("显示 1–20"));
        app.map_search = "另名".into();
        let alias = shown(&ctx, &mut app, Panel::Reverse);
        assert!(alias.contains("1 个匹配对象"), "{alias}");
        assert!(alias.contains("entity:object_044"), "{alias}");
        assert!(!alias.contains("character:object_044"), "{alias}");
        assert_eq!(app.project.content_baseline(), baseline);
        assert!(app.history.is_empty());
    }

    #[test]
    fn marker_and_scene_binding_use_eight_rows_and_reach_the_sixth_page() {
        for panel in [Panel::Marker, Panel::Scene] {
            let (ctx, mut app) = fixture();
            let baseline = app.project.content_baseline();
            app.map_form.target_query = "同名".into();
            app.map_canvas.scene.binding_query = "同名".into();
            if matches!(panel, Panel::Scene) {
                click(&ctx, &mut app, panel, "对象绑定与导航");
            }
            let first = shown(&ctx, &mut app, panel);
            assert!(first.contains("46 个匹配对象 · 显示 1–8"), "{first}");
            assert!(!first.contains("entity:object_007"), "{first}");
            for _ in 0..5 {
                click(&ctx, &mut app, panel, "下一页候选");
            }
            assert!(shown(&ctx, &mut app, panel).contains("显示 41–46"));
            click(
                &ctx,
                &mut app,
                panel,
                if matches!(panel, Panel::Scene) {
                    "同名 · entity:object_044"
                } else {
                    "同名  ·  entity:object_044"
                },
            );
            let target = if matches!(panel, Panel::Scene) {
                &app.map_canvas.scene.inspector.as_ref().unwrap().target_ref
            } else {
                &app.map_form.target
            };
            assert_eq!(target, &Some(TargetRef::new("entity", "object_044")));
            assert_eq!(app.project.content_baseline(), baseline);
            assert!(app.history.is_empty());
        }
    }
    #[test]
    fn search_change_blocks_marker_and_scene_binding_until_explicit_resolution() {
        for panel in [Panel::Marker, Panel::Scene] {
            let (ctx, mut app) = fixture();
            app.map_form.target_query = "另名".into();
            app.map_canvas.scene.binding_query = "另名".into();
            if matches!(panel, Panel::Scene) {
                click(&ctx, &mut app, panel, "对象绑定与导航");
            }
            click(
                &ctx,
                &mut app,
                panel,
                if matches!(panel, Panel::Scene) {
                    "同名 · entity:object_044"
                } else {
                    "同名  ·  entity:object_044"
                },
            );
            app.map_form.target_query = "object_001".into();
            app.map_canvas.scene.binding_query = "object_001".into();
            let changed = shown(&ctx, &mut app, panel);
            assert!(
                changed.contains("待确认引用 entity:object_044"),
                "{changed}"
            );
            if matches!(panel, Panel::Scene) {
                assert!(app
                    .map_canvas
                    .scene
                    .inspector
                    .as_ref()
                    .unwrap()
                    .target_ref
                    .is_none());
                click(&ctx, &mut app, panel, "应用对象修改");
                assert!(app.map_canvas.scene.operations.is_empty());
            } else {
                assert!(app.map_form.target.is_none());
                assert!(!app.marker_binding_ready());
            }
            click(&ctx, &mut app, panel, "不使用对象引用");
            if matches!(panel, Panel::Scene) {
                assert!(app.map_canvas.scene.binding.pending().is_none());
            } else {
                assert!(app.marker_binding_ready());
            }
        }
    }

    #[test]
    fn source_change_invalidates_search_selection_and_reselect_confirms_current_identity() {
        let (ctx, mut app) = fixture();
        app.map_form.target_query = "另名".into();
        click(&ctx, &mut app, Panel::Marker, "同名  ·  entity:object_044");
        app.recompile();
        let rendered = shown(&ctx, &mut app, Panel::Marker);
        assert!(
            rendered.contains("待确认引用 entity:object_044"),
            "{rendered}"
        );
        assert!(app.map_form.target.is_none());
        app.map_form.target_query = "object_001".into();
        click(&ctx, &mut app, Panel::Marker, "同名  ·  entity:object_001");
        assert!(app.marker_binding_ready());
        assert_eq!(
            app.map_form.target,
            Some(TargetRef::new("entity", "object_001"))
        );
    }
}
