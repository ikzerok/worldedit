use super::*;
use egui::{Event, PointerButton, Pos2, RawInput, Rect, Vec2};
use serde_json::{json, Value};
use worldline_core::project::Project;
use worldline_core::queries::{CatalogQueryFilter, PropertyScalar};

struct TempRoot(PathBuf);
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> (TempRoot, egui::Context, WorldeditApp) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = TempRoot(std::env::temp_dir().join(format!(
        "worldedit-query-v3-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )));
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    app.project = Project::new(&root.0);
    app.project
        .set_text(
            &app.project.entry.clone(),
            concat!(
                "event start\n  -> END\n",
                "entity target kind place as \"同名\"\n",
                "character target as \"同名\"\n",
                "entity other kind place as \"同名\"\n",
                "relation_type relates\n",
                "relation_def edge type relates from entity target to entity other\n",
                "entity record_ref kind record\n  property value = ref(\"entity\", \"target\")\n",
                "entity record_character kind record\n  property value = ref(\"character\", \"target\")\n",
                "entity record_relation kind record\n  property value = ref(\"relation\", \"edge\")\n",
                "entity record_string kind record\n  property value = \"entity:target\"\n",
                "entity record_empty kind record\n  property value = \"\"\n",
                "entity record_space kind record\n  property value = \" \"\n",
                "entity record_spaces kind record\n  property value = \"  \"\n",
                "entity record_zero kind record\n  property value = 0\n",
                "entity record_false kind record\n  property value = false\n",
                "entity record_absent kind record\n",
            )
            .into(),
        )
        .unwrap();
    app.project.create_authoring_document(
        &root.0.join(".world/project.json"),
        br#"{"schema_version":1,"language_version":"1.13","entry":"world.wl","required_features":["content.entities.v1","content.relations.v1","content.object_refs.v1","content.character_refs.v1"]}"#.to_vec(),
    ).unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    (root, ctx, app)
}

fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    state: &mut WorkbenchState,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 2200.0))),
            events,
            ..Default::default()
        },
        |ctx| state.render(app, ctx),
    )
}

fn position(shape: &egui::epaint::Shape, label: &str) -> Option<Pos2> {
    match shape {
        egui::epaint::Shape::Text(text) if text.galley.text() == label => {
            Some(text.pos + text.galley.size() * 0.5)
        }
        egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|shape| position(shape, label)),
        _ => None,
    }
}

fn click(ctx: &egui::Context, app: &mut WorldeditApp, state: &mut WorkbenchState, label: &str) {
    let output = frame(ctx, app, state, Vec::new());
    let point = output
        .shapes
        .iter()
        .find_map(|shape| position(&shape.shape, label))
        .unwrap_or_else(|| panic!("未显示 {label}"));
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            state,
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

fn input(kind: ScalarInputKind, value: &str, reference_kind: &str) -> FilterInputs {
    FilterInputs {
        property_key: "value".into(),
        property_kind: kind,
        property_value: value.into(),
        property_reference_kind: Some(reference_kind.into()),
        ..Default::default()
    }
}

fn add_input(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    state: &mut WorkbenchState,
    inputs: FilterInputs,
) {
    filters::push_empty_filter(&mut state.query, "property");
    state.inputs = inputs;
    click(ctx, app, state, "添加属性值");
}

#[test]
fn property_input_uses_core_identity_validation_and_preserves_scalar_bytes() {
    let root = std::path::Path::new(".");
    for kind in ["entity", "relation", "character"] {
        for id in ["target", "END", "_missing"] {
            let condition = property_input::condition_from_inputs(
                &input(ScalarInputKind::Reference, id, kind),
                root,
            )
            .unwrap();
            assert_eq!(
                condition.equals,
                PropertyScalar::Reference(TargetRef::new(kind, id))
            );
        }
        for id in [
            "",
            " ",
            " target",
            "target ",
            "target.id",
            "bad-id",
            "显示名",
        ] {
            assert!(property_input::condition_from_inputs(
                &input(ScalarInputKind::Reference, id, kind),
                root
            )
            .is_err());
        }
    }
    assert!(property_input::condition_from_inputs(
        &input(ScalarInputKind::Reference, "target", "event"),
        root
    )
    .is_err());
    for value in ["", " ", "  ", "entity:target"] {
        let condition = property_input::condition_from_inputs(
            &input(ScalarInputKind::String, value, "entity"),
            root,
        )
        .unwrap();
        assert_eq!(condition.equals, PropertyScalar::String(value.into()));
    }
    for value in ["", " ", "NaN", "inf", "1e999"] {
        assert!(property_input::condition_from_inputs(
            &input(ScalarInputKind::Number, value, "entity"),
            root
        )
        .is_err());
    }
    assert_eq!(
        property_input::value_label(&PropertyScalar::String(String::new())),
        "空字符串"
    );
    assert_eq!(
        property_input::value_label(&PropertyScalar::String(" ".into())),
        "\" \""
    );
    assert_ne!(
        property_input::value_label(&PropertyScalar::String("entity:target".into())),
        property_input::value_label(&PropertyScalar::Reference(TargetRef::new(
            "entity", "target"
        )))
    );
}

#[test]
fn actual_property_add_distinguishes_all_values_without_editing_source() {
    let (_root, ctx, mut app) = fixture();
    let baseline = app.project.content_baseline();
    for (kind, value, target_kind, expected) in [
        (ScalarInputKind::Reference, "target", "entity", "record_ref"),
        (
            ScalarInputKind::Reference,
            "target",
            "character",
            "record_character",
        ),
        (
            ScalarInputKind::Reference,
            "edge",
            "relation",
            "record_relation",
        ),
        (
            ScalarInputKind::String,
            "entity:target",
            "entity",
            "record_string",
        ),
        (ScalarInputKind::String, "", "entity", "record_empty"),
        (ScalarInputKind::String, " ", "entity", "record_space"),
        (ScalarInputKind::String, "  ", "entity", "record_spaces"),
        (ScalarInputKind::Number, "0", "entity", "record_zero"),
        (ScalarInputKind::Boolean, "", "entity", "record_false"),
    ] {
        let mut state = WorkbenchState::default();
        add_input(&ctx, &mut app, &mut state, input(kind, value, target_kind));
        assert_eq!(
            state.query.schema_version,
            if kind == ScalarInputKind::Reference {
                3
            } else {
                1
            }
        );
        let page = app
            .project
            .query_catalog(&state.query, current_options(&state))
            .unwrap();
        assert_eq!(page.items.len(), 1, "{expected}");
        assert_eq!(page.items[0].target.id, expected);
    }
    let mut state = WorkbenchState::default();
    add_input(
        &ctx,
        &mut app,
        &mut state,
        input(ScalarInputKind::Reference, "absent", "entity"),
    );
    assert_eq!(
        app.project
            .query_catalog(&state.query, current_options(&state))
            .unwrap()
            .total,
        0
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn selecting_reference_mode_does_not_upgrade_until_add_and_removal_restores_old_version() {
    let (_root, ctx, mut app) = fixture();
    let mut state = WorkbenchState::default();
    filters::push_empty_filter(&mut state.query, "property");
    click(&ctx, &mut app, &mut state, "文字");
    click(&ctx, &mut app, &mut state, "对象引用（需查询 v3）");
    assert_eq!(state.query.schema_version, 1);
    state.inputs.property_key = "value".into();
    state.inputs.property_value = "target".into();
    click(&ctx, &mut app, &mut state, "添加属性值");
    assert_eq!(state.query.schema_version, 3);
    click(
        &ctx,
        &mut app,
        &mut state,
        "× value = 对象引用 entity:target",
    );
    assert_eq!(state.query.schema_version, 1);
    state.query.set_sort(Some(CatalogQuerySort {
        field: CatalogSortField::Name,
        direction: CatalogSortDirection::Ascending,
    }));
    add_input(
        &ctx,
        &mut app,
        &mut state,
        input(ScalarInputKind::Reference, "target", "entity"),
    );
    assert_eq!(state.query.schema_version, 3);
    click(&ctx, &mut app, &mut state, "清空条件");
    assert_eq!(state.query.schema_version, 2);
    assert!(state.query.sort.is_some());
    assert!(app.history.is_empty());
}

#[test]
fn saved_reference_query_roundtrip_and_explicit_removal_keep_extensions_and_fingerprint() {
    let (root, ctx, mut app) = fixture();
    let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
    let mut state = WorkbenchState::default();
    add_input(
        &ctx,
        &mut app,
        &mut state,
        input(ScalarInputKind::Reference, "target", "entity"),
    );
    state.query.set_sort(Some(CatalogQuerySort {
        field: CatalogSortField::Kind,
        direction: CatalogSortDirection::Descending,
    }));
    state.saved_query_id = "references".into();
    state.saved_query_name = "引用条件".into();
    state.save_query(&mut app);
    assert!(state.error.is_none(), "{:?}", state.error);
    let path = root.0.join(".world/queries/references.json");
    let mut document: Value =
        serde_json::from_slice(app.project.authoring_document(&path).unwrap().bytes()).unwrap();
    assert_eq!(document["schema_version"], 1);
    assert_eq!(document["query"]["schema_version"], 3);
    assert_eq!(
        document["required_features"],
        json!(["catalog.query_sort.v1", "catalog.query_reference_values.v1"])
    );
    document["extra"] = json!({"keep": "target"});
    document["query"]["extension"] = json!({"keep": true});
    app.project
        .set_authoring_document(&path, serde_json::to_vec(&document).unwrap())
        .unwrap();
    app.project.save().unwrap();
    app.project = Project::open(&root.0).unwrap();
    app.recompile();
    let draft = app.project.saved_query_index().queries["references"]
        .draft
        .clone();
    let baseline = app.project.content_baseline();
    let mut reopened = WorkbenchState::default();
    reopened.load_saved_query(&app.project, draft.clone());
    assert_eq!(reopened.query, draft.query);
    assert_eq!(app.project.content_baseline(), baseline);
    click(
        &ctx,
        &mut app,
        &mut reopened,
        "× value = 对象引用 entity:target",
    );
    reopened.save_query(&mut app);
    assert!(reopened.error.is_none(), "{:?}", reopened.error);
    let saved: Value =
        serde_json::from_slice(app.project.authoring_document(&path).unwrap().bytes()).unwrap();
    assert_eq!(saved["query"]["schema_version"], 2);
    assert_eq!(saved["required_features"], json!(["catalog.query_sort.v1"]));
    assert_eq!(saved["extra"], document["extra"]);
    assert_eq!(saved["query"]["extension"], document["query"]["extension"]);
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
    assert_eq!(app.project.language_version(), "1.13");
}

#[test]
fn invalid_or_unknown_saved_definitions_and_external_edits_preserve_original_bytes() {
    for mode in [
        "missing",
        "unknown-feature",
        "unknown-version",
        "old-reference",
        "external",
    ] {
        let (root, ctx, mut app) = fixture();
        let mut state = WorkbenchState::default();
        add_input(
            &ctx,
            &mut app,
            &mut state,
            input(ScalarInputKind::Reference, "target", "entity"),
        );
        state.saved_query_id = "references".into();
        state.saved_query_name = "引用条件".into();
        state.save_query(&mut app);
        let draft = app.project.saved_query_index().queries["references"]
            .draft
            .clone();
        let path = root.0.join(".world/queries/references.json");
        let mut document: Value =
            serde_json::from_slice(app.project.authoring_document(&path).unwrap().bytes()).unwrap();
        match mode {
            "missing" => document["required_features"] = json!([]),
            "unknown-feature" => document["required_features"]
                .as_array_mut()
                .unwrap()
                .push(json!("future.query.v8")),
            "unknown-version" => document["query"]["schema_version"] = json!(99),
            "old-reference" => document["query"]["schema_version"] = json!(1),
            _ => {}
        }
        app.project.save().unwrap();
        let disk_bytes = if mode == "external" {
            b"external bytes".to_vec()
        } else {
            serde_json::to_vec(&document).unwrap()
        };
        std::fs::write(&path, &disk_bytes).unwrap();
        if mode != "external" {
            app.project = Project::open(&root.0).unwrap();
            let mut browsing = WorkbenchState::default();
            browsing.load_saved_query(&app.project, draft);
            assert!(browsing.error.is_some(), "{mode}");
            assert_eq!(browsing.query, CatalogQuery::default());
        }
        let baseline = app.project.content_baseline();
        let original = app
            .project
            .authoring_document(&path)
            .unwrap()
            .bytes()
            .to_vec();
        state.save_query(&mut app);
        assert!(state.error.is_some(), "{mode}");
        assert_eq!(app.project.content_baseline(), baseline, "{mode}");
        assert_eq!(
            app.project.authoring_document(&path).unwrap().bytes(),
            original,
            "{mode}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), disk_bytes, "{mode}");
    }
}

#[test]
fn reference_condition_changes_reject_old_cursors_and_old_versions_stay_unchanged() {
    let (_root, ctx, mut app) = fixture();
    let mut state = WorkbenchState::default();
    add_input(
        &ctx,
        &mut app,
        &mut state,
        input(ScalarInputKind::Reference, "target", "entity"),
    );
    add_input(
        &ctx,
        &mut app,
        &mut state,
        input(ScalarInputKind::Reference, "target", "character"),
    );
    state.page_size = 1;
    let snapshot = app
        .project
        .catalog_scope_snapshot(&state.query, state.max_candidates)
        .unwrap();
    state.page = Some(snapshot.query().page(0, state.page_size).unwrap());
    state.snapshot = Some(std::sync::Arc::new(snapshot));
    state.snapshot_key = Some((app.version, app.map_revision));
    state.snapshot_query = Some(state.query.clone());
    state.snapshot_observation = Some(app.project.catalog_scope_observation_key());
    if let CatalogQueryFilter::Property { values, .. } = &mut state.query.filters[0] {
        values[0].equals = PropertyScalar::Reference(TargetRef::new("relation", "edge"));
    }
    state.next_page(&app);
    assert!(state.page.is_none());
    assert!(state.error.as_ref().unwrap().contains("过期"));
    for version in [1, 2, 99] {
        state.query.schema_version = version;
        assert!(state.query.validate(&app.project.root).is_err());
        assert_eq!(state.query.schema_version, version);
    }
}

#[test]
fn loading_supported_old_queries_does_not_upgrade_and_refreshed_definition_blocks_old_draft() {
    let (root, ctx, mut app) = fixture();
    let mut state = WorkbenchState {
        saved_query_id: "legacy".into(),
        saved_query_name: "旧查询".into(),
        ..Default::default()
    };
    for version in [1, 2] {
        if version == 2 {
            state.query.set_sort(Some(CatalogQuerySort {
                field: CatalogSortField::Name,
                direction: CatalogSortDirection::Ascending,
            }));
        }
        state.save_query(&mut app);
        assert!(state.error.is_none(), "{:?}", state.error);
        let draft = app.project.saved_query_index().queries["legacy"]
            .draft
            .clone();
        let baseline = app.project.content_baseline();
        state.load_saved_query(&app.project, draft);
        frame(&ctx, &mut app, &mut state, Vec::new());
        assert_eq!(state.query.schema_version, version);
        assert_eq!(app.project.content_baseline(), baseline);
    }
    app.project.save().unwrap();
    let path = root.0.join(".world/queries/legacy.json");
    let mut external: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    external["name"] = json!("外部新名称");
    let bytes = serde_json::to_vec(&external).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    app.project = Project::open(&root.0).unwrap();
    let baseline = app.project.content_baseline();
    state.saved_query_name = "尚未提交的名称".into();
    state.save_query(&mut app);
    assert!(state.error.as_ref().unwrap().contains("StaleBaseline"));
    assert_eq!(state.saved_query_name, "尚未提交的名称");
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(
        app.project.authoring_document(&path).unwrap().bytes(),
        bytes
    );
}
