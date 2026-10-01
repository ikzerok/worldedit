use super::*;

fn complete_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    let mut native_frame = eframe::Frame::_new_kittest();
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1188.0, 848.0))),
            events,
            ..Default::default()
        },
        |ctx| eframe::App::update(app, ctx, &mut native_frame),
    )
}

#[test]
fn persisted_300_comments_keep_manuscript_input_responsive_and_unapplied() {
    let (ctx, mut app) = manuscript_app();
    add_notes(&mut app, 300);
    app.project.save().unwrap();
    for at in 0..300 {
        assert!(app
            .project
            .root
            .join(format!(".world/comments/revision_{at:03}.json"))
            .is_file());
    }
    let path = app.active_file.clone();
    app.project = Project::open(&path).unwrap();
    app.saved_location = true;
    app.reset_views();
    app.recompile();
    let baseline = app.project.content_baseline();
    let disk_source = std::fs::read(&path).unwrap();
    let original = app.project.document(&path).unwrap().to_owned();
    app.tab = Tab::Manuscript;
    complete_frame(&ctx, &mut app, vec![]);
    let (target, buffer_path) = app.manuscript.active_writing_target().unwrap();
    assert_eq!(target, TargetRef::new("event", "arrival"));
    assert_eq!(buffer_path, path);
    let cursor = original.find("甲乙").unwrap() + "甲乙".len();
    crate::app::search::request_selection(&ctx, path.clone(), original.clone(), cursor..cursor);
    complete_frame(&ctx, &mut app, vec![]);
    complete_frame(&ctx, &mut app, vec![Event::Text("x".into())]);
    let output = complete_frame(&ctx, &mut app, vec![]);
    let buffer = app.manuscript.writing_buffer_mut(&path).unwrap();
    assert!(buffer.is_changed());
    assert_eq!(buffer.source(), original.replacen("甲乙", "甲乙x", 1));
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("未应用草稿"), "{rendered}");
    assert!(rendered.contains("甲乙x"), "{rendered}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&path).unwrap(), original);
    assert_eq!(std::fs::read(&path).unwrap(), disk_source);
    assert!(app.history.is_empty());
}
