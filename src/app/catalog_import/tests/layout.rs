use super::*;

fn tab_to(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let mut trace = Vec::new();
    for _ in 0..150 {
        // A newly opened menu may already focus its first item. Observe that real
        // focus before moving again; never request focus or synthesize a click.
        let mut output = frame(ctx, app, egui::vec2(1188.0, 848.0), vec![]);
        if focused_widget(ctx, label) {
            // Production theme enables 0.12s scroll animation. Let that real
            // transition settle before comparing painted and response geometry.
            for _ in 0..12 {
                output = frame(ctx, app, egui::vec2(1188.0, 848.0), vec![]);
            }
        }
        let focused = ctx.memory(|m| m.focused());
        let response = focused.and_then(|id| ctx.read_response(id));
        if focused_widget(ctx, label)
            && response.as_ref().is_some_and(|r| {
                output.shapes.iter().any(|s| {
                    point(&s.shape, label)
                        .is_some_and(|p| s.clip_rect.contains(p) && r.rect.contains(p))
                })
            })
        {
            return;
        }
        if label.starts_with("属性键")
            && focused.is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some())
        {
            let candidates: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(text) if text.galley.job.text.contains("属性键") => {
                        Some(format!(
                            "text={:?}, pos={:?}, galley={:?}, clip={:?}, point={:?}",
                            text.galley.job.text,
                            text.pos,
                            text.galley.rect,
                            s.clip_rect,
                            point(&s.shape, label)
                        ))
                    }
                    _ => None,
                })
                .collect();
            println!("HINT_PROBE focused_widget={} focused={focused:?} load_state=true response={:?} candidates={candidates:?}", focused_widget(ctx, label), response.as_ref().map(|r| r.rect));
        }
        if trace.len() < 35 {
            trace.push(format!(
                "focus={focused:?}, rect={:?}, popup={}",
                response.as_ref().map(|r| r.rect),
                egui::Popup::is_any_open(ctx)
            ));
        }
        key(ctx, app, Key::Tab);
    }
    let output = frame(ctx, app, egui::vec2(1188.0, 848.0), vec![]);
    panic!("键盘焦点不能到达 {label}: {trace:?}; {}", rendered(&output));
}

#[test]
fn real_buttons_refresh_acknowledge_apply_and_read_source() {
    let (ctx, mut app) = app();
    load(&ctx, &mut app, CSV);
    map(&mut app);
    click(&ctx, &mut app, "刷新预览");
    wait(&ctx, &mut app);
    assert!(app.history.is_empty());
    click(&ctx, &mut app, "已审阅整批字段差异与存档影响");
    click(&ctx, &mut app, "确认整批应用");
    assert_eq!(app.history.len(), 1, "{:?}", app.catalog_import.error);
    click(&ctx, &mut app, "看资料");
    assert_eq!(app.reading_target.as_ref().unwrap().id, "traveler");
    click(&ctx, &mut app, "回到来源");
    assert_eq!(app.tab, Tab::Edit);
    app.author_back(&ctx);
    assert_eq!(app.tab, Tab::CatalogImport);
    assert!(app.catalog_import.plan.is_some());
}

#[test]
fn narrow_wide_light_dark_keep_actions_visible_and_rows_scroll_separately() {
    for size in [egui::vec2(1188.0, 848.0), egui::vec2(760.0, 620.0)] {
        for dark in [true, false] {
            let (ctx, mut app) = app();
            app.personal.settings.theme = if dark {
                crate::theme::ThemeMode::Dark
            } else {
                crate::theme::ThemeMode::Light
            };
            let csv = CSV.replacen("kind,id", &("非常长的中文表头".repeat(12) + ",id"), 1);
            load(&ctx, &mut app, &csv);
            map(&mut app);
            preview(&ctx, &mut app);
            app.catalog_import.source_name =
                "非常长的中文目录与来源文件名/".repeat(16) + "资料.csv";
            for step in [Step::Mapping, Step::Review] {
                app.catalog_import.step = step;
                for _ in 0..5 {
                    let output = frame(&ctx, &mut app, size, vec![]);
                    let screen = Rect::from_min_size(Pos2::ZERO, size);
                    for label in [
                        "世界资料导入",
                        "选择 CSV 快照…",
                        "刷新预览",
                        "确认整批应用",
                        "丢弃导入输入…",
                    ] {
                        assert!(
                            output.shapes.iter().any(|s| point(&s.shape, label)
                                .is_some_and(|p| s.clip_rect.contains(p) && screen.contains(p))),
                            "不可见 {label} size={size:?} dark={dark}: {}",
                            rendered(&output)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn keyboard_can_reach_review_and_apply_but_ime_enter_cannot_submit() {
    let (ctx, mut app) = app();
    load(&ctx, &mut app, CSV);
    map(&mut app);
    tab_to(&ctx, &mut app, "刷新预览");
    key(&ctx, &mut app, Key::Enter);
    wait(&ctx, &mut app);
    assert!(
        app.catalog_import.plan.is_some(),
        "{:?}",
        app.catalog_import.error
    );
    tab_to(&ctx, &mut app, "已审阅整批字段差异与存档影响");
    key(&ctx, &mut app, Key::Space);
    assert!(app.catalog_import.acknowledged);
    tab_to(&ctx, &mut app, "确认整批应用");
    frame(
        &ctx,
        &mut app,
        egui::vec2(1188.0, 848.0),
        vec![
            Event::Ime(egui::ImeEvent::Preedit("输入".into())),
            Event::Key {
                key: Key::Enter,
                physical_key: Some(Key::Enter),
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            },
        ],
    );
    assert!(app.history.is_empty());
    frame(
        &ctx,
        &mut app,
        egui::vec2(1188.0, 848.0),
        vec![Event::Ime(egui::ImeEvent::Commit("输入".into()))],
    );
    assert!(app.history.is_empty());
    click(&ctx, &mut app, "确认整批应用");
    assert_eq!(app.history.len(), 1);
}

#[test]
fn escape_returns_without_discarding_snapshot_and_mapping() {
    let (ctx, mut app) = app();
    load(&ctx, &mut app, CSV);
    map(&mut app);
    let signature = app.catalog_import.input_signature();
    key(&ctx, &mut app, Key::Escape);
    assert_eq!(app.tab, Tab::Catalog);
    assert_eq!(app.catalog_import.input_signature(), signature);
    app.open_catalog_import();
    assert_eq!(app.catalog_import.input_signature(), signature);
}

#[test]
fn keyboard_maps_every_column_and_destination_before_previewing() {
    let (ctx, mut app) = app();
    load(
        &ctx,
        &mut app,
        "类别,编号,中文名称,年龄\ncharacter,reader,海港读者,22\n",
    );
    assert!(app.catalog_import.columns.iter().all(Option::is_none));
    assert!(app.catalog_import.destination.as_os_str().is_empty());
    tab_to(&ctx, &mut app, "选择新对象的源码文件…");
    key(&ctx, &mut app, Key::Enter);
    tab_to(&ctx, &mut app, "world.wl");
    key(&ctx, &mut app, Key::Enter);
    assert_eq!(app.catalog_import.destination, PathBuf::from("world.wl"));
    for label in [
        "对象种类 · kind",
        "稳定 ID · id",
        "显示名 · display",
        "属性 · 数值 number",
    ] {
        tab_to(&ctx, &mut app, "尚未映射");
        key(&ctx, &mut app, Key::Enter);
        tab_to(&ctx, &mut app, label);
        key(&ctx, &mut app, Key::Enter);
    }
    assert!(app.catalog_import.columns.iter().all(Option::is_some));
    tab_to(&ctx, &mut app, "属性键（明确填写，例如 age）");
    frame(
        &ctx,
        &mut app,
        egui::vec2(1188.0, 848.0),
        vec![Event::Text("age".into())],
    );
    tab_to(&ctx, &mut app, "报错（默认）");
    key(&ctx, &mut app, Key::Enter);
    tab_to(&ctx, &mut app, "保留现值 / 跳过");
    key(&ctx, &mut app, Key::Enter);
    assert_eq!(
        app.catalog_import.columns[3].as_ref().unwrap().blank,
        Blank::Keep
    );
    tab_to(&ctx, &mut app, "刷新预览");
    key(&ctx, &mut app, Key::Enter);
    wait(&ctx, &mut app);
    let plan = app.catalog_import.plan.as_ref().unwrap();
    assert!(plan.can_apply, "{plan:?}");
    assert_eq!(plan.rows[0].target.as_ref().unwrap().id, "reader");
    assert!(app.history.is_empty());
}
