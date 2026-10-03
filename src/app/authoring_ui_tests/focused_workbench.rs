//! 自动egui证据不冒充native、物理IME或读屏。
use super::*;
use worldline_core::world_context::WorldContextKind;

pub(super) fn focus_app() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app();
    let manifest = app.project.root.join(".world/project.json");
    app.project.set_authoring_document(&manifest,br#"{"schema_version":1,"language_version":"1.13","required_features":["content.entities.v1","content.relations.v1","content.object_refs.v1","content.character_refs.v1"]}"#.to_vec()).unwrap();
    let text="character linqi as \"林栖\"\n  property mentor = ref(\"character\", \"lingzhou\")\ncharacter lingzhou as \"绫舟与南方群岛灯塔守望者共同保管最后一盏风灯的人\"\nentity lighthouse kind place as \"北灯塔\"\nrelation_type guards as \"守卫\"\nrelation_def lamp type guards from character lingzhou to entity lighthouse\nperiod night\nevent witness during night with linqi\n  [[character:lingzhou|绫舟]] 在绫舟港见到灯火。\n  -> END\nevent lights_out during night follows witness\n  -> END\n";
    app.project
        .set_text(&app.active_file.clone(), text.into())
        .unwrap();
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.diagnostics()
    );
    app.tab = Tab::Characters;
    app.select_character("linqi");
    (ctx, app)
}
pub(super) fn work_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)),
            events,
            ..Default::default()
        },
        |ctx| {
            crate::theme::configure(ctx, app.personal.settings.theme);
            app.author_shortcuts(ctx);
            app.top_bar(ctx);
            app.status_bar(ctx);
            app.sidebar(ctx);
            app.characters_tab(ctx);
            app.temporal_issues_window(ctx);
            app.capture_edit_focus(ctx);
        },
    )
}
fn text(output: &egui::FullOutput) -> String {
    let mut text = String::new();
    for s in &output.shapes {
        collect_text(&s.shape, &mut text);
    }
    text
}
#[test]
fn focused_context_uses_typed_core_projection_and_mentions_are_opt_in() {
    let (ctx, mut app) = focus_app();
    work_frame(&ctx, &mut app, vec2(1188.0, 848.0), vec![]);
    let result = app.character_focus.result.as_ref().unwrap();
    assert!(result
        .records
        .iter()
        .any(|r| r.kind == WorldContextKind::PropertyReference));
    assert!(result
        .records
        .iter()
        .any(|r| r.kind == WorldContextKind::EventParticipation));
    assert!(!app.character_focus.mentions);
    assert!(result
        .records
        .iter()
        .all(|r| r.kind != WorldContextKind::TextMention));
    assert!(result.nodes.iter().any(|n| n.target.id == "linqi"));
    let expected = app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .query_world_context(&TargetRef::new("character", "linqi"), Default::default())
        .unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}
#[test]
fn three_sizes_keep_focus_and_actions_visible_without_empty_inspector() {
    for size in [
        vec2(1400.0, 900.0),
        vec2(1188.0, 848.0),
        vec2(1040.0, 660.0),
    ] {
        for mode in [
            crate::theme::ThemeMode::Dark,
            crate::theme::ThemeMode::Light,
        ] {
            let (ctx, mut app) = focus_app();
            app.personal.settings.theme = mode;
            app.select_character("lingzhou");
            for _ in 0..3 {
                work_frame(&ctx, &mut app, size, vec![]);
            }
            let out = work_frame(&ctx, &mut app, size, vec![]);
            assert!(visible_text_position(&out, "适配当前结果").is_some());
            assert!(
                visible_text_position(&out, "当前焦点").is_some(),
                "size={size:?} text={}",
                text(&out)
            );
            assert!(visible_text_position(&out, "编辑档案 / 旧式关系").is_some());
            assert!(ctx
                .memory(|m| m.area_rect(egui::Id::new("character-details-drawer")))
                .is_none());
            let position = app.character_focus.positions["character:lingzhou"];
            assert_eq!(position, [0.0, 0.0]);
        }
    }
}
#[test]
fn dirty_character_survives_navigation_panel_and_escape() {
    let (ctx, mut app) = focus_app();
    app.character_editor.as_mut().unwrap().draft.display = "未应用的林栖".into();
    let before = app.project.content_baseline();
    app.select_character("lingzhou");
    assert_eq!(
        app.character_editor.as_ref().unwrap().original.as_deref(),
        Some("linqi")
    );
    work_frame(&ctx, &mut app, vec2(1040.0, 660.0), vec![]);
    app.open_temporal_issues(&ctx);
    for _ in 0..3 {
        work_frame(&ctx, &mut app, vec2(1040.0, 660.0), vec![]);
    }
    assert_eq!(ctx.memory(|m| m.focused()), app.temporal_issues.entry_focus);
    work_frame(
        &ctx,
        &mut app,
        vec2(1040.0, 660.0),
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(!app.temporal_issues.open);
    assert_eq!(
        app.character_editor.as_ref().unwrap().draft.display,
        "未应用的林栖"
    );
    assert_eq!(app.project.content_baseline(), before);
}
#[test]
fn source_navigation_returns_same_character_filter_and_camera() {
    let (ctx, mut app) = focus_app();
    work_frame(&ctx, &mut app, vec2(1188.0, 848.0), vec![]);
    app.character_focus.filters[4] = false;
    app.character_focus.camera.pan = [42.0, 13.0];
    let row = app.character_focus.result.as_ref().unwrap().records[0].clone();
    app.jump_to_file(
        &row.source.file,
        row.source.line,
        row.source.column.unwrap_or(1),
    );
    assert_eq!(app.tab, Tab::Edit);
    app.author_back(&ctx);
    assert_eq!(app.tab, Tab::Characters);
    assert_eq!(
        app.character_editor.as_ref().unwrap().original.as_deref(),
        Some("linqi")
    );
    assert!(!app.character_focus.filters[4]);
    assert_eq!(app.character_focus.camera.pan, [42.0, 13.0]);
}
#[test]
fn recompile_does_not_leave_stale_context_facts() {
    let (ctx, mut app) = focus_app();
    work_frame(&ctx, &mut app, vec2(1188.0, 848.0), vec![]);
    let old = app
        .character_focus
        .result
        .as_ref()
        .unwrap()
        .snapshot
        .clone();
    let source = app
        .project
        .document(&app.active_file)
        .unwrap()
        .replace("  property mentor = ref(\"character\", \"lingzhou\")\n", "");
    app.project
        .set_text(&app.active_file.clone(), source)
        .unwrap();
    app.recompile();
    work_frame(&ctx, &mut app, vec2(1188.0, 848.0), vec![]);
    let result = app.character_focus.result.as_ref().unwrap();
    assert_ne!(result.snapshot, old);
    assert!(!result
        .records
        .iter()
        .any(|r| r.kind == WorldContextKind::PropertyReference));
}
#[test]
fn temporal_problem_groups_and_comparison_consume_current_core() {
    let (ctx, mut app) = focus_app();
    app.project.set_text(&app.active_file.clone(),"period night\nevent witness during night follows lights_out\n  -> END\nevent lights_out during night follows witness\n  -> END\nevent lighthouse during night follows lights_out\n  -> END\nevent cave during night follows lighthouse\n  -> END\n".into()).unwrap();
    app.recompile();
    app.open_temporal_issues(&ctx);
    app.temporal_issues.left = "witness".into();
    app.temporal_issues.right = "cave".into();
    for _ in 0..3 {
        let _ = ctx.run(RawInput::default(), |ctx| app.temporal_issues_window(ctx));
    }
    let out = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1188.0, 848.0))),
            ..Default::default()
        },
        |ctx| app.temporal_issues_window(ctx),
    );
    let content = text(&out);
    assert!(content.contains("1 个真正时间环"));
    assert!(content.contains("2 个受阻下游"));
    let timeline = &app.snapshot.as_ref().unwrap().result.analysis.timeline;
    assert_eq!(timeline.cycles[0].members.len(), 2);
    assert_eq!(timeline.blocked.len(), 2);
    assert_eq!(
        timeline.compare("witness", "cave").relation,
        worldline_core::timeline::TemporalRelation::Invalid
    );
    let source = app.project.document(&app.active_file).unwrap().replace(
        "witness during night follows lights_out",
        "witness during night",
    );
    app.project
        .set_text(&app.active_file.clone(), source)
        .unwrap();
    app.recompile();
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .timeline
        .cycles
        .is_empty());
    assert_eq!(
        app.snapshot
            .as_ref()
            .unwrap()
            .result
            .analysis
            .timeline
            .compare("witness", "cave")
            .relation,
        worldline_core::timeline::TemporalRelation::Before
    );
}
#[test]
fn large_person_index_retains_exact_last_selection_and_bounded_projection() {
    for count in [100, 1000] {
        let (ctx, mut app) = focus_app();
        let mut source = String::new();
        for index in 0..count {
            source.push_str(&format!("character person_{index} as \"同名人物\"\n"));
        }
        source.push_str("event start\n  -> END\n");
        app.project
            .set_text(&app.active_file.clone(), source)
            .unwrap();
        app.recompile();
        app.character_editor = None;
        app.select_character(&format!("person_{}", count - 1));
        for _ in 0..3 {
            work_frame(&ctx, &mut app, vec2(1040.0, 660.0), vec![]);
        }
        assert_eq!(
            app.character_editor.as_ref().unwrap().original.as_deref(),
            Some(format!("person_{}", count - 1).as_str())
        );
        assert_eq!(
            app.character_focus.result.as_ref().unwrap().target.id,
            format!("person_{}", count - 1)
        );
        app.character_focus.query = "person_0".into();
        work_frame(&ctx, &mut app, vec2(1040.0, 660.0), vec![]);
        assert_eq!(
            app.character_focus.result.as_ref().unwrap().target.id,
            format!("person_{}", count - 1)
        );
    }
}

#[test]
fn inspector_drawer_keeps_long_content_and_main_action_reachable() {
    for size in [vec2(1188.0, 848.0), vec2(1040.0, 660.0)] {
        let (ctx, mut app) = focus_app();
        app.select_character("lingzhou");
        app.character_focus.inspector_open = true;
        app.character_editor
            .as_mut()
            .unwrap()
            .draft
            .relations
            .push(("linqi".into(), "长关系标签".repeat(20)));
        for _ in 0..4 {
            work_frame(&ctx, &mut app, size, vec![]);
        }
        let rect = ctx
            .memory(|m| m.area_rect(egui::Id::new("character-details-drawer")))
            .unwrap();
        assert!(
            Rect::from_min_size(pos2(0.0, 0.0), size).contains_rect(rect),
            "{rect:?}"
        );
        let name = app.character_editor.as_ref().unwrap().draft.display.clone();
        app.character_focus.inspector_open = false;
        work_frame(&ctx, &mut app, size, vec![]);
        assert_eq!(app.character_editor.as_ref().unwrap().draft.display, name);
        assert_eq!(
            app.character_editor.as_ref().unwrap().draft.relations[0]
                .1
                .chars()
                .count(),
            100
        );
    }
}
#[test]
fn region_shortcut_reaches_graph_results_and_details_without_erasing_input() {
    let (ctx, mut app) = focus_app();
    for (zone, id) in [
        (1, "character-context-canvas"),
        (2, "character-context-results-entry"),
        (3, "character-name-input"),
        (0, "character-index-query-input"),
    ] {
        work_frame(
            &ctx,
            &mut app,
            vec2(1040.0, 660.0),
            vec![Event::Key {
                key: egui::Key::F6,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(app.character_focus.zone, zone);
        assert_eq!(ctx.memory(|m| m.focused()), Some(egui::Id::new(id)));
    }
}
#[test]
fn formal_and_explicit_link_kinds_remain_read_only_and_legacy_creation_is_explicit() {
    let (ctx, mut app) = focus_app();
    app.select_character("lingzhou");
    work_frame(&ctx, &mut app, vec2(1188.0, 848.0), vec![]);
    let records = &app.character_focus.result.as_ref().unwrap().records;
    for kind in [
        WorldContextKind::FormalRelation,
        WorldContextKind::ExplicitBodyLink,
        WorldContextKind::PropertyReference,
    ] {
        assert!(records.iter().any(|r| r.kind == kind), "missing {kind:?}");
    }
    assert!(app
        .character_editor
        .as_ref()
        .unwrap()
        .draft
        .relations
        .is_empty());
    let before = app.project.content_baseline();
    app.character_focus.mentions = true;
    work_frame(&ctx, &mut app, vec2(1188.0, 848.0), vec![]);
    assert_eq!(app.project.content_baseline(), before);
    assert!(app
        .character_editor
        .as_ref()
        .unwrap()
        .draft
        .relations
        .is_empty());
}

#[test]
fn legacy_port_drag_creates_only_legacy_edge_and_keeps_property_reference() {
    let (ctx, mut app) = focus_app();
    for _ in 0..4 {
        work_frame(&ctx, &mut app, vec2(1188.0, 848.0), vec![]);
    }
    let canvas = app.character_focus.canvas.unwrap();
    let size = [canvas.width() as f64, canvas.height() as f64];
    let screen = |id: &str| {
        let p = app.character_focus.camera.world_to_canvas(
            app.character_focus.positions[&format!("character:{id}")],
            size,
        );
        canvas.min + vec2(p[0] as f32, p[1] as f32)
    };
    let start = screen("linqi")
        + vec2(
            100.0 * (app.character_focus.camera.zoom as f32).max(1.0),
            0.0,
        );
    let end = screen("lingzhou");
    work_frame(
        &ctx,
        &mut app,
        vec2(1188.0, 848.0),
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    work_frame(
        &ctx,
        &mut app,
        vec2(1188.0, 848.0),
        vec![Event::PointerMoved(end)],
    );
    work_frame(
        &ctx,
        &mut app,
        vec2(1188.0, 848.0),
        vec![
            Event::PointerMoved(end),
            Event::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    let info = &app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .symbols
        .characters["linqi"];
    assert_eq!(info.relations.len(), 1);
    assert_eq!(info.relations[0].target, "lingzhou");
    assert!(info.properties.contains_key("mentor"));
    assert!(app.character_link.is_none());
}

#[test]
fn region_shortcut_does_not_cross_settings_guards_or_ime() {
    for layer in ["preferences", "guard", "ime"] {
        let (ctx, mut app) = focus_app();
        work_frame(&ctx, &mut app, vec2(1040.0, 660.0), vec![]);
        match layer {
            "preferences" => app.personal.preferences_open = true,
            "guard" => app.new_file = Some("unapplied.wl".into()),
            _ => app.ime_composing = true,
        }
        let zone = app.character_focus.zone;
        work_frame(
            &ctx,
            &mut app,
            vec2(1040.0, 660.0),
            vec![Event::Key {
                key: egui::Key::F6,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(app.character_focus.zone, zone, "layer {layer}");
    }
}

#[test]
fn conflicted_focus_shows_buffer_scope_without_overwriting_either_version() {
    let (ctx, mut app) = focus_app();
    app.project.save().unwrap();
    let path = app.active_file.clone();
    let original = app.project.document(&path).unwrap().to_owned();
    let local = original.replace("林栖", "林栖本地稿");
    let disk = original.replace("林栖", "林栖磁盘稿");
    app.project.set_text(&path, local.clone()).unwrap();
    std::fs::write(&path, &disk).unwrap();
    app.recompile();
    work_frame(&ctx, &mut app, vec2(1188.0, 848.0), vec![]);
    let result = app.character_focus.result.as_ref().unwrap();
    assert!(!result.complete);
    assert!(!result.truncated);
    assert!(result
        .reasons
        .contains(&worldline_core::world_context::WorldContextLimit::SourceConflict));
    assert!(app
        .character_focus
        .source_status
        .as_ref()
        .unwrap()
        .contains("缓冲快照"));
    assert!(result
        .records
        .iter()
        .any(|r| r.kind == WorldContextKind::PropertyReference));
    assert_eq!(app.project.document(&path).unwrap(), local);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), disk);
    let _ = std::fs::remove_dir_all(app.project.root);
}
