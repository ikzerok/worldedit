//! 真实 egui 帧与鼠标回归；不等同于操作系统窗口实机验收。
use crate::app::{Tab, WorldeditApp};
use egui::{pos2, vec2, Event, FullOutput, PointerButton, RawInput, Rect};
use worldline_core::project::Project;

fn app() -> (egui::Context, WorldeditApp, std::path::PathBuf) {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = std::env::temp_dir().join(format!(
        "worldedit-root-timeline-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.project = Project::new(&root);
    let entry = app.project.entry.clone();
    app.project.documents.retain(|p, _| p == &entry);
    app.project.set_text(&entry, "period year\nperiod summer within year\nperiod autumn within year\nevent opening during summer\n  开幕\n".into()).unwrap();
    let closing_file = app
        .project
        .add_file(std::path::Path::new("chapters/closing.wl"))
        .unwrap();
    app.project
        .set_text(
            &closing_file,
            "event closing during autumn follows opening\n  闭幕\n".into(),
        )
        .unwrap();
    app.project
        .create_authoring_document(
            &root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.13","required_features":[]}"#.to_vec(),
        )
        .unwrap();
    app.active_file = entry;
    app.tab = Tab::Timeline;
    app.reset_views();
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.snapshot.as_ref().unwrap().result.diagnostics
    );
    (ctx, app, closing_file)
}
fn frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) -> FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
            events,
            ..Default::default()
        },
        |ctx| app.canvas_tab(ctx),
    )
}
fn texts(shape: &egui::Shape, out: &mut Vec<(String, egui::Pos2)>) {
    match shape {
        egui::Shape::Text(text) => out.push((
            text.galley.job.text.clone(),
            text.pos + text.galley.rect.center().to_vec2(),
        )),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                texts(shape, out);
            }
        }
        _ => {}
    }
}
fn rendered(ctx: &egui::Context, app: &mut WorldeditApp) -> Vec<(String, egui::Pos2)> {
    let mut text = Vec::new();
    for _ in 0..4 {
        let output = frame(ctx, app, Vec::new());
        text.clear();
        for shape in output.shapes {
            texts(&shape.shape, &mut text);
        }
    }
    text
}
fn click(ctx: &egui::Context, app: &mut WorldeditApp, point: egui::Pos2) {
    for pressed in [true, false] {
        frame(
            ctx,
            app,
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
fn native_root_columns_keep_direct_bands_and_double_click_cross_file_source() {
    let (ctx, mut app, closing_file) = app();
    let baseline = app.project.content_baseline();
    let text = rendered(&ctx, &mut app);
    let point = |label: &str| {
        text.iter()
            .find(|(s, _)| s == label)
            .unwrap_or_else(|| panic!("未显示{label}：{text:?}"))
            .1
    };
    let opening = point("opening");
    let closing = point("closing");
    assert!(closing.x > opening.x + 200.0, "跨时段应按根层级分列");
    assert!(closing.y > opening.y, "直接时段分组必须保留");
    assert!(text.iter().any(|(s, _)| s.contains("独立根不可比")));
    assert!(text
        .iter()
        .any(|(s, _)| s.contains("summer · 1 个直属事件")));
    assert!(text
        .iter()
        .any(|(s, _)| s.contains("autumn · 1 个直属事件")));
    assert!(text.iter().any(|(s, _)| s == "跨时段先于"));
    click(&ctx, &mut app, closing);
    click(&ctx, &mut app, closing);
    assert_eq!(app.tab, Tab::Edit);
    assert_eq!(app.active_file, closing_file);
    assert_eq!(app.jump, Some((1, 1)));
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn native_empty_invalid_graph_and_unknown_period_keep_partial_warning_visible() {
    let (ctx, mut app, closing_file) = app();
    app.project.set_text(&closing_file, String::new()).unwrap();
    for source in ["period\n", "event unplaced during missing\n  日期未知\n"] {
        app.project
            .set_text(&app.active_file.clone(), source.into())
            .unwrap();
        app.recompile();
        app.tab = Tab::Timeline;
        let text = rendered(&ctx, &mut app);
        assert!(
            text.iter().any(|(s, _)| s.contains("时间线不完整")),
            "{text:?}"
        );
        if source.contains("unplaced") {
            assert!(
                text.iter().any(|(s, _)| s == "unplaced"),
                "无效时段不应藏掉事件"
            );
            assert!(text.iter().any(|(s, _)| s.contains("层级未知")));
        }
    }
}

#[test]
fn native_link_accepts_shared_root_and_rejects_separate_roots_without_writes() {
    for shared_root in [true, false] {
        let (ctx, mut app, closing_file) = app();
        app.project
            .set_text(
                &closing_file,
                "event closing during autumn\n  闭幕\n".into(),
            )
            .unwrap();
        if !shared_root {
            let entry = app.active_file.clone();
            let source = app
                .project
                .document(&entry)
                .unwrap()
                .replace("period autumn within year", "period autumn");
            app.project.set_text(&entry, source).unwrap();
        }
        app.recompile();
        app.link_from = Some("opening".into());
        let text = rendered(&ctx, &mut app);
        assert!(text.iter().any(|(s, _)| s.contains("同一时间根")));
        let closing = text.iter().find(|(s, _)| s == "closing").unwrap().1;
        let before = app.project.sources();
        let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
        click(&ctx, &mut app, closing);
        if shared_root {
            assert!(app.link_from.is_none());
            assert!(app
                .project
                .document(&closing_file)
                .unwrap()
                .contains("follows opening"));
            let result = &app.snapshot.as_ref().unwrap().result;
            assert_eq!(result.analysis.fingerprint, fingerprint);
            assert_eq!(
                result
                    .analysis
                    .timeline
                    .events
                    .iter()
                    .find(|e| e.event == "closing")
                    .unwrap()
                    .root_rank,
                Some(1)
            );
        } else {
            assert_eq!(app.project.sources(), before);
            assert!(
                app.io_error
                    .as_ref()
                    .is_some_and(|error| error.contains("A213")),
                "{:?}",
                app.io_error
            );
            assert_eq!(app.link_from.as_deref(), Some("opening"));
        }
    }
}
