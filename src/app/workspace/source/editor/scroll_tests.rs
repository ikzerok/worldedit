//! native长行搜索反例：真实App布局、280px问题区、默认滚动动画，不能只测即时滚动。
use crate::app::{Tab, WorldeditApp};
use egui::{Context, Event, Key, Modifiers};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

fn frame(
    ctx: &Context,
    app: &mut WorldeditApp,
    tick: &mut u32,
    events: Vec<Event>,
) -> egui::FullOutput {
    *tick += 1;
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1188.0, 848.0),
            )),
            time: Some(f64::from(*tick) / 60.0),
            events,
            ..Default::default()
        },
        |ctx| eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest()),
    )
}

fn press(ctx: &Context, app: &mut WorldeditApp, tick: &mut u32, key: Key, modifiers: Modifiers) {
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            tick,
            vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            }],
        );
    }
}

#[test]
fn source_wrap_full_app_animated_search_reaches_long_line_end_above_problem_panel() {
    let ctx = Context::default();
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root =
        std::env::temp_dir().join(format!("worldedit-long-wrap-search-{}", std::process::id()));
    let source = format!(
        "world glass_tide as \"玻璃潮与六座钟楼\"\ncharacter keeper as \"守钟人\"\nevent entrance as \"第六钟楼的门\"\n  {}WRAP_END_018 {}\n  choice \"进入钟楼\"\n    -> missing_ending\n  choice \"返回潮岸\"\n    -> END\n",
        "玻璃潮卷过石阶，守钟人沿着旧地图寻找第七声。🌊 ".repeat(160), "ABCDEFGHIJKLMNOPQRSTUVWXYZ".repeat(12),
    );
    let files = BTreeMap::from([
        (PathBuf::from("world.wl"), source.as_bytes().to_vec()),
        (
            PathBuf::from(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.13","required_features":[]}"#.to_vec(),
        ),
    ]);
    app.project =
        worldline_core::project::Project::from_snapshot(&root, Path::new("world.wl"), &files)
            .unwrap();
    app.active_file = app.project.entry.clone();
    app.tab = Tab::Edit;
    app.personal.settings.source_wrap = true;
    app.personal.settings.source_size = 16.0;
    app.personal.settings.diagnostics = true;
    app.recompile();
    ctx.data_mut(|data| {
        data.insert_persisted(
            egui::Id::new("project-problems"),
            egui::containers::panel::PanelState {
                rect: egui::Rect::from_min_size(egui::pos2(0.0, 568.0), egui::vec2(1188.0, 280.0)),
            },
        )
    });
    let mut tick = 0;
    for _ in 0..60 {
        frame(&ctx, &mut app, &mut tick, vec![]);
    }
    let id = egui::Id::new(("source", &app.active_file));
    let mut state = egui::TextEdit::load_state(&ctx, id).unwrap();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(0),
        )));
    state.store(&ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
    frame(&ctx, &mut app, &mut tick, vec![]);
    press(&ctx, &mut app, &mut tick, Key::F, Modifiers::COMMAND);
    frame(
        &ctx,
        &mut app,
        &mut tick,
        vec![Event::Text("WRAP_END_018".into())],
    );
    press(&ctx, &mut app, &mut tick, Key::Enter, Modifiers::NONE);
    assert!(app.search_open);
    press(&ctx, &mut app, &mut tick, Key::Escape, Modifiers::NONE);
    assert!(!app.search_open);
    let mut trace = Vec::new();
    for _ in 0..90 {
        frame(&ctx, &mut app, &mut tick, vec![]);
        if let Some((_, view)) = &app.personal.source_view {
            let view = serde_json::to_value(view).unwrap();
            trace.push((
                app.personal.source_scroll[1],
                view["layout"]["height"].clone(),
                view["anchor"].clone(),
            ));
        }
    }
    let output = frame(&ctx, &mut app, &mut tick, vec![]);
    let selected = egui::TextEdit::load_state(&ctx, id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    let index = source[..source.find("WRAP_END_018").unwrap()]
        .chars()
        .count();
    assert_eq!(
        selected.as_sorted_char_range(),
        index..index + "WRAP_END_018".chars().count()
    );
    let (visible, cursor, rows) = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == source => Some((
                shape.clip_rect,
                text.galley
                    .pos_from_cursor(selected.primary)
                    .translate(text.pos.to_vec2()),
                text.galley.rows.len(),
            )),
            _ => None,
        })
        .expect("全App源码galley必须仍可见");
    assert!(rows > 50);
    assert!(
        visible.bottom() < 600.0,
        "保持真实280问题区，不靠扩大窗口通过：{visible:?}"
    );
    assert!(
        visible.contains(cursor.center()),
        "长行命中必须滚入源码视口：viewport={visible:?}, cursor={cursor:?}, trace={trace:?}"
    );
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    assert!(!app.project.is_dirty());
}
