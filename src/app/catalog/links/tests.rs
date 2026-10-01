use super::*;
use egui::{pos2, vec2, Event, PointerButton, RawInput, Rect};

fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    target: &TargetRef,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 1000.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.object_links(ui, target);
            });
        },
    )
}
fn find(shape: &egui::Shape, text: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(value) if value.galley.job.text == text => {
            Some(value.pos + value.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(values) => values.iter().find_map(|shape| find(shape, text)),
        _ => None,
    }
}
fn collect(shape: &egui::Shape, out: &mut String) {
    match shape {
        egui::Shape::Text(value) => out.push_str(&value.galley.job.text),
        egui::Shape::Vec(values) => {
            for shape in values {
                collect(shape, out);
            }
        }
        _ => {}
    }
}

#[test]
fn template_only_character_backlink_uses_cached_index_and_matches_delete_impact() {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = std::env::temp_dir().join(format!(
        "template-only-backlink-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.project = worldline_core::project::Project::new(&root);
    let entry = app.project.entry.clone();
    app.project.documents.retain(|path, _| path == &entry);
    app.project
        .set_text(
            &entry,
            "character lin as \"林舟\"\nentity boat kind ship\nevent start\n  -> END\n".into(),
        )
        .unwrap();
    app.project.create_authoring_document(&root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.13","required_features":["content.object_refs.v1","content.character_refs.v1","content.templates.v1"],"templates":{"project:ship":".world/templates/ship.json"}}"#.to_vec()).unwrap();
    app.project.create_authoring_document(&root.join(".world/templates/ship.json"), br#"{"schema_version":1,"id":"project:ship","title":"Ship","required_features":["content.character_refs.v1"],"applies_to":{"kind":"entity"},"fields":[{"id":"captain_id","key":"captain","label":"Captain","type":"object_ref","required":false,"target":{"kind":"character"},"default":{"kind":"character","id":"lin"}}]}"#.to_vec()).unwrap();
    app.recompile();
    let target = TargetRef::new("character", "lin");
    let impact = app.project.deletion_impact(&target);
    assert!(impact.content_references.is_empty());
    assert!(impact.complete);
    assert!(!impact.can_delete());
    assert_eq!(impact.template_references.len(), 1);
    let cached = app
        .snapshot
        .as_ref()
        .unwrap()
        .template_index
        .references_to(&target);
    assert_eq!(
        serde_json::to_value(&cached).unwrap(),
        serde_json::to_value(&impact.template_references).unwrap()
    );
    let baseline = app.project.content_baseline();
    for _ in 0..3 {
        frame(&ctx, &mut app, &target, Vec::new());
    }
    let output = frame(&ctx, &mut app, &target, Vec::new());
    let point = output
        .shapes
        .iter()
        .find_map(|shape| find(&shape.shape, "引用来源 · 1 处"))
        .expect("模板引用应计入引用来源数量");
    for pressed in [true, false] {
        frame(
            &ctx,
            &mut app,
            &target,
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
    let output = frame(&ctx, &mut app, &target, Vec::new());
    let mut text = String::new();
    for shape in &output.shapes {
        collect(&shape.shape, &mut text);
    }
    assert!(text.contains("模板默认值引用"), "{text}");
    assert!(text.contains(".world/templates/ship.json"), "{text}");
    assert!(!text.contains("尚无直接引用"), "{text}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}
