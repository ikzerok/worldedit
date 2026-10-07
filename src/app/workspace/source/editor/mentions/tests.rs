use super::*;
use egui::{Event, Key};
fn key(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn mention_pages_reach_1500_by_real_page_keys_and_clear_stale_visibility() {
    let (ctx, mut app, _) = super::super::tests::app();
    app.snapshot
        .as_mut()
        .unwrap()
        .result
        .analysis
        .catalog
        .objects = (0..1500)
        .map(|index| CatalogObject {
            target: TargetRef::new("entity", &format!("item_{index:04}")),
            display: "同名".into(),
            file: app.active_file.to_string_lossy().into_owned(),
            line: index as u32 + 1,
        })
        .collect();
    let path = app.active_file.clone();
    let baseline = app.project.content_baseline();
    let frame = |app: &mut WorldeditApp, events| {
        let mut last = None;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                last = Some(app.mention_page(ctx, &path, 0, "同名", true));
                app.mention_popup(ctx, &path, egui::pos2(40.0, 80.0), 0, "同名", 0);
            },
        );
        last.unwrap()
    };
    frame(&mut app, vec![]);
    for _ in 0..74 {
        frame(&mut app, vec![key(Key::PageDown)]);
    }
    let last = frame(&mut app, vec![]);
    assert_eq!(last.items.len(), 20);
    assert_eq!(
        last.items.last().unwrap().target,
        TargetRef::new("entity", "item_1499")
    );
    assert!(!last.visible.is_empty());
    let id = app.mention_id(&path);
    let state = ctx.data(|data| data.get_temp::<MentionState>(id)).unwrap();
    assert_eq!(state.page.result.unwrap().unwrap().total, 1500);
    app.version += 1;
    let refreshed = frame(&mut app, vec![key(Key::Enter)]);
    assert!(refreshed.stale);
    assert!(refreshed.visible.is_empty());
    assert_eq!(
        refreshed.items[0].target,
        TargetRef::new("entity", "item_0000")
    );
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn query_and_filter_changes_reset_page_and_never_leave_old_visible_candidates() {
    let (ctx, mut app, _) = super::super::tests::app();
    let path = app.active_file.clone();
    app.mention_page(&ctx, &path, 0, "harbor", false);
    let id = app.mention_id(&path);
    let mut state = ctx.data(|data| data.get_temp::<MentionState>(id)).unwrap();
    state.visible.push(TargetRef::new("entity", "harbor"));
    ctx.data_mut(|data| data.insert_temp(id, state));
    let absent = app.mention_page(&ctx, &path, 0, "无匹配", false);
    assert!(absent.items.is_empty());
    assert!(absent.visible.is_empty());
    let mut state = ctx.data(|data| data.get_temp::<MentionState>(id)).unwrap();
    state.page.options.max_candidates = 1;
    ctx.data_mut(|data| data.insert_temp(id, state));
    let error = app.mention_page(&ctx, &path, 0, "", false);
    assert!(error.items.is_empty());
    assert!(ctx
        .data(|data| data.get_temp::<MentionState>(id))
        .unwrap()
        .page
        .result
        .unwrap()
        .is_err());
}

#[test]
fn ime_commit_can_measure_popup_without_exposing_an_actionable_invisible_row() {
    let (ctx, mut app, _) = super::super::tests::app();
    let path = app.active_file.clone();
    app.snapshot
        .as_mut()
        .unwrap()
        .result
        .analysis
        .catalog
        .objects = vec![CatalogObject {
        target: TargetRef::new("entity", "harbor"),
        display: "潮港".into(),
        file: path.to_string_lossy().into_owned(),
        line: 1,
    }];
    let frame = |app: &mut WorldeditApp, events| {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                app.mention_page(ctx, &path, 0, "潮港", false);
                assert!(app
                    .mention_popup(ctx, &path, egui::pos2(40.0, 80.0), 0, "潮港", 0)
                    .is_none());
            },
        );
        ctx.data(|data| data.get_temp::<MentionState>(app.mention_id(&path)))
            .unwrap()
    };
    let commit = frame(
        &mut app,
        vec![
            Event::Ime(egui::ImeEvent::Commit("潮港".into())),
            key(Key::Enter),
        ],
    );
    assert!(
        commit.visible.is_empty(),
        "IME禁用／首次不可见测量行不能成为Enter可确认目标"
    );
    let ready = frame(&mut app, vec![]);
    assert_eq!(ready.visible, vec![TargetRef::new("entity", "harbor")]);
}
