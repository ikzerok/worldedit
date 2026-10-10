//! 当前轮 egui 行为回归；组合事件为合成输入，不替代物理 IME 或原生桌面验收。
mod baseline_handoff;
mod baseline_handoff_guards;
mod body_apply_discard;
mod context_menu;
mod cross_feature_undo;
mod discard_lifecycle;
mod edges;
mod history_cases;
mod history_guards;
mod narrow;
#[cfg(not(target_arch = "wasm32"))]
mod save_refresh_history;
use super::world_links::Kind;
use super::*;
use crate::app::{Tab, WorldeditApp};
use egui::{pos2, vec2, Event, RawInput, Rect};
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};
use worldline_core::project::Project;

const TEXT: &str = "character lin as \"林芜😀\"\ncharacter other as \"林芜😀\"\nevent start\n  我和林芜😀走向灯塔。\n  -> END\nevent second\n  第二章文字。\n  -> END\n";
fn fixture() -> (egui::Context, WorldeditApp) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "worldedit-writing-links-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".world/manuscripts")).unwrap();
    fs::write(root.join("world.wl"), TEXT).unwrap();
    fs::write(root.join("资料.wl"), "// 新资料所在文件\n").unwrap();
    fs::write(root.join("unrelated.wl"), "// 其他文件\n").unwrap();
    fs::write(root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.9","entry":"world.wl","required_features":["presentation.manuscripts.v1"],"manuscripts":{"book":".world/manuscripts/book.json"}}"#).unwrap();
    fs::write(root.join(".world/manuscripts/book.json"), br#"{"schema_version":1,"id":"book","title":"Book","entries":[{"id":"first","kind":"chapter","title":"First","target_ref":{"kind":"event","id":"start"}},{"id":"second","kind":"chapter","title":"Second","target_ref":{"kind":"event","id":"second"}}]}"#).unwrap();
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    app.project = Project::open(&root).unwrap();
    app.active_file = app.project.entry.clone();
    app.reset_views();
    app.recompile();
    app.tab = Tab::Manuscript;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    select_name(&ctx, &mut app);
    (ctx, app)
}
fn frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
    frame_size(ctx, app, vec2(1280.0, 1000.0), events)
}
fn frame_size(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    events: Vec<Event>,
) -> egui::FullOutput {
    let mut raw = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)),
        events,
        ..Default::default()
    };
    app.manuscript_raw_input_hook(ctx, &mut raw);
    ctx.run(raw, |ctx| {
        let _theme = crate::theme::configure_appearance(ctx, app.personal.appearance());
        app.manuscript_tab(ctx);
    })
}
fn select_name(ctx: &egui::Context, app: &mut WorldeditApp) {
    let path = app.project.entry.clone();
    let buffer = app.manuscript.writing_buffers.get(&path).unwrap();
    let offset = buffer.source().find("我和林芜😀").unwrap();
    let id = egui::Id::new(("writing-prose", &path, "event", "start", offset));
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(2),
            egui::text::CCursor::new(5),
        )));
    state.store(ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
    frame(ctx, app, vec![]);
    let selected = crate::app::search::editor_selection(ctx).unwrap();
    assert_eq!(&selected.source[selected.range], "林芜😀");
}
fn text_shapes<'a>(shape: &'a egui::Shape, out: &mut Vec<&'a egui::epaint::TextShape>) {
    match shape {
        egui::Shape::Text(text) => out.push(text),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                text_shapes(shape, out);
            }
        }
        _ => {}
    }
}
fn labels(output: &egui::FullOutput) -> String {
    output
        .shapes
        .iter()
        .flat_map(|shape| {
            let mut text = Vec::new();
            text_shapes(&shape.shape, &mut text);
            text
        })
        .map(|text| text.galley.text())
        .collect::<Vec<_>>()
        .join("\n")
}
fn control_position(
    ctx: &egui::Context,
    output: &egui::FullOutput,
    label: &str,
) -> Option<egui::Pos2> {
    output.shapes.iter().find_map(|shape| {
        let mut texts = Vec::new();
        text_shapes(&shape.shape, &mut texts);
        texts.into_iter().find_map(|text| {
            let rect = text.galley.rect.translate(text.pos.to_vec2());
            (text.galley.text() == label
                && shape.clip_rect.contains_rect(rect)
                && ctx.screen_rect().contains_rect(rect))
            .then_some(rect.center())
        })
    })
}
fn settled_controls(ctx: &egui::Context, app: &mut WorldeditApp) -> egui::FullOutput {
    frame(ctx, app, vec![]);
    frame(ctx, app, vec![]);
    frame(ctx, app, vec![])
}
fn click_point(ctx: &egui::Context, app: &mut WorldeditApp, pos: egui::Pos2) {
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}
fn click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let mut output = settled_controls(ctx, app);
    // 只有这些已收拢的正文操作走渐进菜单；资料窗口等其它目标缺失仍立即失败。
    let body_action = matches!(
        label,
        "选词工具"
            | "应用正文草稿"
            | "应用源码草稿（可含诊断）"
            | "丢弃此文件草稿"
            | "取消丢弃"
            | "确认丢弃正文草稿"
    );
    if body_action && control_position(ctx, &output, label).is_none() {
        assert_eq!(app.tab, Tab::Manuscript);
        assert!(
            !app.manuscript
                .world_links
                .as_ref()
                .is_some_and(|state| state.open),
            "不能从已打开的资料表单盲点背后的正文工具"
        );
        let key = egui::Id::new("world-links-test-open-body-tools");
        let saved = ctx.data(|data| data.get_temp::<egui::Id>(key));
        let popup = if let Some(id) = saved.filter(|id| egui::Popup::is_id_open(ctx, *id)) {
            id
        } else {
            assert!(
                !egui::Popup::is_any_open(ctx),
                "不能关闭其它 popup 来寻找 {label}"
            );
            let pos = control_position(ctx, &output, "正文工具")
                .unwrap_or_else(|| panic!("找不到可见正文工具以到达 {label}: {}", labels(&output)));
            click_point(ctx, app, pos); // 只真实展开一轮，不写 popup 状态或编辑状态。
            let id = ctx
                .interaction_snapshot(|snapshot| snapshot.clicked)
                .expect("须实际点到正文工具");
            let response = ctx.read_response(id).expect("刚点击的正文工具必须实际存在");
            let popup = egui::Popup::default_response_id(&response);
            assert!(egui::Popup::is_id_open(ctx, popup));
            ctx.data_mut(|data| data.insert_temp(key, popup));
            output = settled_controls(ctx, app);
            popup
        };
        for _ in 0..32 {
            if control_position(ctx, &output, label).is_some() {
                break;
            }
            assert!(
                egui::Popup::is_id_open(ctx, popup),
                "正文菜单已关闭：{label}"
            );
            let rect = ctx
                .memory(|memory| memory.area_rect(popup))
                .expect("真实正文 popup")
                .intersect(ctx.screen_rect());
            let pos = ["应用正文草稿", "应用源码草稿（可含诊断）", "丢弃此文件草稿"]
                .iter()
                .find_map(|label| control_position(ctx, &output, label))
                .unwrap_or(egui::pos2(rect.center().x, rect.bottom() - 12.0));
            assert_eq!(
                ctx.layer_id_at(pos).map(|layer| layer.id),
                Some(popup),
                "滚轮必须确实命中已验证的正文 popup，不能滚其它窗口"
            );
            frame(
                ctx,
                app,
                vec![
                    Event::PointerMoved(pos),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, -24.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            output = settled_controls(ctx, app);
        }
    }
    let pos = control_position(ctx, &output, label)
        .unwrap_or_else(|| panic!("找不到完整可见按钮 {label}: {}", labels(&output)));
    click_point(ctx, app, pos);
}
fn key(key: egui::Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
fn new_character(app: &mut WorldeditApp) {
    let state = app.manuscript.world_links.as_mut().unwrap();
    state.kind = Kind::Character;
    state.id = "new_person".into();
    state.destination = app.project.root.join("资料.wl");
    state.touched = true;
}

#[test]
fn actual_selection_existing_link_preview_stays_draft_and_same_name_does_not_guess() {
    let (ctx, mut app) = fixture();
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, "选词工具");
    click(&ctx, &mut app, "关联世界资料…");
    frame(&ctx, &mut app, vec![]);
    assert!(app
        .manuscript
        .world_links
        .as_ref()
        .unwrap()
        .chosen
        .is_none());
    let page = app
        .manuscript
        .world_links
        .as_ref()
        .unwrap()
        .page
        .result
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap();
    assert_eq!(page.total, 2);
    app.manuscript.world_links.as_mut().unwrap().chosen = Some(TargetRef::new("character", "lin"));
    click(&ctx, &mut app, "预览关联计划");
    assert!(app.manuscript.world_links.as_ref().unwrap().plan.is_some());
    assert_eq!(app.project.content_baseline(), baseline);
    click(&ctx, &mut app, "插入引用到正文草稿");
    assert!(app.manuscript.world_links.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    let path = app.project.entry.clone();
    assert!(app.manuscript.writing_buffers[&path]
        .source()
        .contains("[[character:lin|林芜😀]]"));
    app.edit_undo(false);
    assert_eq!(app.manuscript.writing_buffers[&path].source(), TEXT);
    app.edit_undo(true);
    assert!(app.manuscript.writing_buffers[&path]
        .source()
        .contains("[[character:lin|林芜😀]]"));
}

#[test]
fn compound_character_applies_explicit_full_drafts_once_and_restores_them_on_undo() {
    let (ctx, mut app) = fixture();
    let path = app.project.entry.clone();
    app.manuscript
        .writing_buffers
        .get_mut(&path)
        .unwrap()
        .replace_source(TEXT.replace("走向", "慢慢走向"));
    frame(&ctx, &mut app, vec![]);
    select_name(&ctx, &mut app);
    let source_before = app.manuscript.writing_buffers[&path].source().to_owned();
    let destination = app.project.root.join("资料.wl");
    let mut destination_buffer = app
        .project
        .open_source_writing_buffer(&destination)
        .unwrap();
    destination_buffer.replace_source("// 目标文件已有未应用说明\n".into());
    let unrelated_path = app.project.root.join("unrelated.wl");
    let mut unrelated = app
        .project
        .open_source_writing_buffer(&unrelated_path)
        .unwrap();
    unrelated.replace_source("event broken\n  if (\n".into());
    app.manuscript
        .restore_writing_buffers(&[destination_buffer.clone(), unrelated.clone()]);
    app.begin_manuscript_world_links(&ctx);
    new_character(&mut app);
    let mut state = app.manuscript.world_links.take().unwrap();
    app.preview_manuscript_world_link(&mut state);
    let plan = state
        .plan
        .as_ref()
        .unwrap_or_else(|| panic!("{:?}", state.error));
    assert_eq!(plan.included_buffers.len(), 2);
    assert!(plan.included_buffers.iter().all(|buffer| buffer.changed));
    assert_eq!(plan.changes.len(), 2);
    let baseline = app.project.content_baseline();
    assert!(
        app.apply_manuscript_world_link(&ctx, &mut state),
        "{:?}",
        state.error
    );
    assert_eq!(app.history.len(), 1);
    assert!(app
        .project
        .document(&destination)
        .unwrap()
        .contains("character new_person"));
    assert!(app.project.document(&path).unwrap().contains("慢慢走向"));
    assert_eq!(
        app.manuscript.writing_buffers[&unrelated_path].source(),
        unrelated.source()
    );
    assert_eq!(
        app.manuscript.writing_buffers[&unrelated_path].baseline(),
        app.project.content_baseline()
    );
    app.edit_undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(
        app.manuscript.writing_buffers[&path].source(),
        source_before
    );
    assert_eq!(
        app.manuscript.writing_buffers[&destination].source(),
        destination_buffer.source()
    );
    assert_eq!(
        app.manuscript.writing_buffers[&unrelated_path].source(),
        unrelated.source()
    );
    app.edit_undo(true);
    assert!(app
        .project
        .document(&destination)
        .unwrap()
        .contains("character new_person"));
    assert!(!app.apply_manuscript_world_link(&ctx, &mut state));
    app.project.save().unwrap();
    let reopened = Project::open(&app.project.root).unwrap();
    assert!(reopened
        .compile_object_search_snapshot()
        .analysis
        .catalog
        .object(&TargetRef::new("character", "new_person"))
        .is_some());
    assert_eq!(
        reopened
            .compile_object_search_snapshot()
            .analysis
            .catalog
            .text_links
            .len(),
        1
    );
}

#[test]
fn close_and_escape_keep_inputs_and_return_original_chapter_selection() {
    let (ctx, mut app) = fixture();
    app.begin_manuscript_world_links(&ctx);
    new_character(&mut app);
    let before = app.project.content_baseline();
    click(&ctx, &mut app, "返回正文，保留输入");
    assert!(!app.manuscript.world_links.as_ref().unwrap().open);
    assert_eq!(
        app.manuscript.world_links.as_ref().unwrap().id,
        "new_person"
    );
    app.begin_manuscript_world_links(&ctx);
    frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
    assert!(!app.manuscript.world_links.as_ref().unwrap().open);
    assert!(app.manuscript.has_unsubmitted_work());
    assert_eq!(app.project.content_baseline(), before);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(
        app.manuscript.books["book"].selected_entry.as_deref(),
        Some("first")
    );
    assert_eq!(
        app.manuscript_session()
            .cursor
            .as_ref()
            .map(|cursor| cursor.cursor),
        Some(5)
    );
}

#[test]
fn peek_and_return_preserve_same_chapter_and_do_not_apply() {
    let (ctx, mut app) = fixture();
    app.begin_manuscript_world_links(&ctx);
    app.manuscript.world_links.as_mut().unwrap().chosen = Some(TargetRef::new("character", "lin"));
    let before = app.project.content_baseline();
    click(&ctx, &mut app, "旁查资料");
    assert_eq!(app.reading_target, Some(TargetRef::new("character", "lin")));
    assert!(!app.manuscript.world_links.as_ref().unwrap().open);
    click(&ctx, &mut app, "返回正文关联");
    frame(&ctx, &mut app, vec![]);
    assert!(app.manuscript.world_links.as_ref().unwrap().open);
    assert!(app.reading_target.is_none());
    assert_eq!(
        app.manuscript.books["book"].selected_entry.as_deref(),
        Some("first")
    );
    assert_eq!(app.project.content_baseline(), before);
}

#[test]
fn stale_buffer_or_changed_form_refuses_previewed_application_without_losing_input() {
    let (ctx, mut app) = fixture();
    app.begin_manuscript_world_links(&ctx);
    new_character(&mut app);
    let mut state = app.manuscript.world_links.take().unwrap();
    app.preview_manuscript_world_link(&mut state);
    assert!(state.plan.is_some());
    let old = app.project.content_baseline();
    state.id = "changed_person".into();
    assert!(!app.apply_manuscript_world_link(&ctx, &mut state));
    assert_eq!(state.id, "changed_person");
    assert_eq!(app.project.content_baseline(), old);
    app.preview_manuscript_world_link(&mut state);
    let buffer = app
        .manuscript
        .writing_buffers
        .get_mut(&app.project.entry)
        .unwrap();
    buffer.replace_source(format!("{}\n// 后续输入\n", buffer.source()));
    assert!(!app.apply_manuscript_world_link(&ctx, &mut state));
    assert!(state.error.is_some());
    assert_eq!(app.project.content_baseline(), old);
}

#[test]
fn synthetic_ime_commit_enter_escape_do_not_close_or_apply_association() {
    let (ctx, mut app) = fixture();
    app.begin_manuscript_world_links(&ctx);
    new_character(&mut app);
    frame(&ctx, &mut app, vec![]);
    let id = egui::Id::new(("world-link-field", "显示名称"));
    ctx.memory_mut(|memory| memory.request_focus(id));
    frame(&ctx, &mut app, vec![]);
    let baseline = app.project.content_baseline();
    frame(
        &ctx,
        &mut app,
        vec![
            Event::Ime(egui::ImeEvent::Enabled),
            Event::Ime(egui::ImeEvent::Preedit("中文".into())),
        ],
    );
    frame(
        &ctx,
        &mut app,
        vec![
            Event::Ime(egui::ImeEvent::Commit("中文".into())),
            key(egui::Key::Enter),
            key(egui::Key::Escape),
        ],
    );
    let state = app.manuscript.world_links.as_ref().unwrap();
    assert!(state.open);
    assert!(state.display.contains("中文"));
    assert!(state.plan.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn entity_migration_is_visible_and_part_of_the_single_project_undo() {
    let (ctx, mut app) = fixture();
    app.begin_manuscript_world_links(&ctx);
    let mut state = app.manuscript.world_links.take().unwrap();
    state.kind = Kind::Entity;
    state.id = "tower".into();
    app.preview_manuscript_world_link(&mut state);
    assert!(state.plan.is_none());
    assert_eq!(app.project.language_version(), "1.9");
    state.enable_entities = true;
    app.preview_manuscript_world_link(&mut state);
    assert!(state.plan.as_ref().unwrap().migration.is_some());
    assert_eq!(app.project.language_version(), "1.9");
    assert!(app.apply_manuscript_world_link(&ctx, &mut state));
    assert_eq!(app.project.language_version(), "1.10");
    assert_eq!(app.history.len(), 1);
    app.edit_undo(false);
    assert_eq!(app.project.language_version(), "1.9");
}
