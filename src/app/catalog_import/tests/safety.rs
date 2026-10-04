use super::*;

#[test]
fn disk_conflict_at_apply_keeps_both_versions_and_no_history() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    load(&ctx, &mut app, CSV);
    map(&mut app);
    preview(&ctx, &mut app);
    let before = app.project.sources();
    let disk = "character traveler as \"磁盘新稿\"\nevent opening\n  -> END\n";
    std::fs::write(&app.active_file, disk).unwrap();
    apply(&mut app);
    assert_eq!(app.project.sources(), before);
    assert_eq!(std::fs::read_to_string(&app.active_file).unwrap(), disk);
    assert!(app.history.is_empty());
    assert!(app.catalog_import.stale);
    assert!(app.catalog_import.error.is_some());
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn clean_open_character_form_refreshes_instead_of_resurrecting_old_input() {
    let (ctx, mut app) = app();
    app.select_character("traveler");
    assert!(app.dirty_draft_names().is_empty());
    load(&ctx, &mut app, CSV);
    map(&mut app);
    preview(&ctx, &mut app);
    apply(&mut app);
    assert_eq!(
        app.character_editor.as_ref().unwrap().draft.display,
        "远行旅人"
    );
    assert!(
        app.dirty_draft_names().is_empty(),
        "{:?}",
        app.dirty_draft_names()
    );
}

#[test]
fn missing_capability_is_visible_and_never_upgrades_the_project() {
    let (ctx, mut app) = app();
    let before = app.project.content_baseline();
    load(
        &ctx,
        &mut app,
        "kind,id,display,entity_type\nentity,harbor,海港,place\n",
    );
    app.catalog_import.columns = [Field::Kind, Field::Id, Field::Display, Field::EntityType]
        .into_iter()
        .enumerate()
        .map(|(column, field)| {
            Some(CatalogColumnMapping {
                column,
                field,
                blank: Blank::Error,
            })
        })
        .collect();
    app.catalog_import.destination = PathBuf::from("world.wl");
    preview(&ctx, &mut app);
    let plan = app.catalog_import.plan.as_ref().unwrap();
    assert!(!plan.can_apply);
    assert!(plan
        .diagnostics
        .iter()
        .any(|d| d.code == "IMPORT_CAPABILITY"));
    click(&ctx, &mut app, "显式启用语言与资料能力…");
    assert!(app.capability_ui.is_some());
    assert_eq!(app.project.content_baseline(), before);
    assert_eq!(app.project.language_version(), "1.9");
    assert!(app.catalog_import.table.is_some());
}

#[test]
fn mixed_kind_typed_fields_blank_keep_and_forward_refs_share_the_core_plan() {
    let (ctx, mut app) = app();
    app.project.create_authoring_document(&app.project.root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.13","required_features":["content.entities.v1","content.object_refs.v1","content.character_refs.v1"]}"#.to_vec()).unwrap();
    app.recompile();
    let csv = "kind,id,display,type,description,age,ready,note,friend\ncharacter,traveler,旅人,,,21,true,新文字,guide\ncharacter,guide,向导,,,35,false,同行,traveler\nentity,harbor,海港,place,潮汐港口,2,true,海雾,traveler\n";
    load(&ctx, &mut app, csv);
    let fields = [
        Field::Kind,
        Field::Id,
        Field::Display,
        Field::EntityType,
        Field::Description,
        Field::Property {
            key: "age".into(),
            value_type: Type::Number,
        },
        Field::Property {
            key: "ready".into(),
            value_type: Type::Bool,
        },
        Field::Property {
            key: "note".into(),
            value_type: Type::Text,
        },
        Field::Property {
            key: "friend".into(),
            value_type: Type::Ref {
                target_kind: "character".into(),
            },
        },
    ];
    app.catalog_import.columns = fields
        .into_iter()
        .enumerate()
        .map(|(column, field)| {
            Some(CatalogColumnMapping {
                column,
                field,
                blank: if column == 3 || column == 4 {
                    Blank::Keep
                } else {
                    Blank::Error
                },
            })
        })
        .collect();
    app.catalog_import.destination = PathBuf::from("world.wl");
    preview(&ctx, &mut app);
    let plan = app.catalog_import.plan.as_ref().unwrap();
    assert!(plan.can_apply, "{plan:?}");
    assert_eq!(plan.rows.len(), 3);
    apply(&mut app);
    assert!(
        app.catalog_import.error.is_none(),
        "{:?}",
        app.catalog_import.error
    );
    assert_eq!(app.history.len(), 1);
    let catalog = &app.snapshot.as_ref().unwrap().result.analysis.catalog;
    assert_eq!(catalog.entities["harbor"].description, "潮汐港口");
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .symbols
        .characters
        .contains_key("guide"));
}
