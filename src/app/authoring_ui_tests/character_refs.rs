use super::*;

#[test]
fn tag_property_editor_can_fill_and_update_character_typed_slot() {
    let (ctx, mut app) = app();
    let manifest = app.project.root.join(".world/project.json");
    app.project.set_authoring_document(&manifest, br#"{"schema_version":1,"language_version":"1.13","required_features":["content.object_refs.v1","content.character_refs.v1"]}"#.to_vec()).unwrap();
    let path = app.active_file.clone();
    app.project.set_text(&path, "character lin as \"林舟\"\ncharacter mei as \"梅\"\ntag crew as \"船员\"\n  property captain = ref(\"character\", \"lin\")\nevent start\n  -> END\n".into()).unwrap();
    app.recompile();
    app.catalog_target = Some(TargetRef::new("tag", "crew"));
    app.tab = Tab::Catalog;
    click(&ctx, &mut app, 14, "标签属性");
    click(&ctx, &mut app, 14, "林舟 · character:lin");
    let text = rendered_text_in_window(&ctx, &mut app, 14, "梅 · 人物:mei");
    assert!(text.contains("梅 · 人物:mei"), "{text}");
    click_containing(&ctx, &mut app, 14, "梅 · 人物:mei");
    assert_eq!(
        app.tag_editor.as_ref().unwrap().1.properties[0].1,
        worldline_core::ast::PropertyValue::Ref(TargetRef::new("character", "mei"))
    );
    click(&ctx, &mut app, 14, "应用标签资料");
    assert!(app
        .project
        .document(&path)
        .unwrap()
        .contains("property captain = ref(\"character\", \"mei\")"));
    assert!(!app.project.compile().has_errors());
}
