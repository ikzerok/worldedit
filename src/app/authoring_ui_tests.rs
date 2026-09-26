//! 真实 egui 窗口按钮回归；不是已显示桌面或浏览器的人工验收。
use super::WorldeditApp;
use egui::{pos2, vec2, Event, PointerButton, RawInput, Rect};
use std::sync::atomic::{AtomicUsize, Ordering};
use worldline_core::{project::Project, TargetRef};

static NEXT_TEST_ROOT: AtomicUsize = AtomicUsize::new(0);

fn app() -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = std::env::temp_dir().join(format!(
        "worldedit-form-ui-{}-{}",
        std::process::id(),
        NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    app.project = Project::new(&root);
    let entry = app.project.entry.clone();
    app.project.documents.retain(|path, _| path == &entry);
    app.project.set_text(&entry, "entity a kind place as \"同名\"\nentity b kind organization as \"同名\"\nrelation_type knows as \"认识\"\n".into()).unwrap();
    app.project.create_authoring_document(&root.join(".world/project.json"), br#"{
        "schema_version":1,"language_version":"1.10",
        "required_features":["content.entities.v1","content.relations.v1"],"maps":{},"graph_views":{}
    }"#.to_vec()).unwrap();
    app.active_file = entry;
    app.reset_views();
    app.recompile();
    (ctx, app)
}
fn manuscript_app() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app();
    app.project
        .set_text(
            &app.active_file.clone(),
            concat!(
                "character traveler as \"旅人\"\n",
                "event arrival as \"抵达\"\n",
                "  甲乙 [[character:traveler|林澈]]\n",
                "  scene harbor\n",
                "    灯塔亮起。\n",
                "    -> END\n",
                "  -> END\n",
                "event departure as \"离港\"\n",
                "  远航。\n",
                "  -> END\n",
                "entity a kind place as \"同名\"\n",
                "  description \"同名地点资料。\"\n",
                "entity b kind organization as \"同名\"\n",
                "  description \"同名组织资料。\"\n",
            )
            .into(),
        )
        .unwrap();
    app.project
        .set_authoring_document(
            &app.project.root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.10","entry":"world.wl","required_features":["content.entities.v1","content.relations.v1","presentation.manuscripts.v1"],"maps":{},"graph_views":{},"manuscripts":{"novel":".world/manuscripts/novel.json"}}"#.to_vec(),
        )
        .unwrap();
    app.project
        .create_authoring_document(
            &app.project.root.join(".world/manuscripts/novel.json"),
            r#"{"schema_version":1,"id":"novel","title":"雾港书稿","entries":[{"id":"opening","kind":"chapter","title":"抵达","target_ref":{"kind":"event","id":"arrival"},"summary":"旅人来到港口","status":"draft","goal":"100"},{"id":"departure","kind":"chapter","title":"离港","target_ref":{"kind":"event","id":"departure"},"status":"planned","goal":"50"},{"id":"harbor","kind":"chapter","title":"港口","target_ref":{"kind":"scene","id":"arrival.harbor"}},{"id":"notes","kind":"chapter","title":"同名资料","target_ref":{"kind":"entity","id":"a"}}]}"#.as_bytes().to_vec(),
        )
        .unwrap();
    app.reset_views();
    app.recompile();
    (ctx, app)
}
fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
    window: u8,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                if window == 14 {
                    vec2(700.0, 640.0)
                } else if window == 9 {
                    vec2(1040.0, 660.0)
                } else {
                    vec2(1700.0, 1400.0)
                },
            )),
            events,
            ..Default::default()
        },
        |ctx| match window {
            0 => app.entity_editor_window(ctx),
            1 => app.relation_editor_window(ctx),
            2 => app.relation_type_editor_window(ctx),
            4 => app.network_tab(ctx),
            5 => app.target_rename_window(ctx),
            6 => app.preset_editor_window(ctx),
            7 => app.review_tab(ctx),
            14 => app.review_tab(ctx),
            8 | 9 => app.reading_window(ctx),
            11 => app.source_tab(ctx),
            13 => app.manuscript_tab(ctx),
            10 => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    app.reading_content(ui, TargetRef::new("entity", "a"));
                });
            }
            12 => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let target = app.reading_target.clone().unwrap();
                    app.reading_content(ui, target);
                });
            }
            14 => app.catalog_tab(ctx),
            _ => app.content_deletion_window(ctx),
        },
    )
}

#[test]
fn catalog_query_opens_from_the_existing_catalog_and_keeps_browsing_read_only() {
    let (ctx, mut app) = app();
    app.tab = super::Tab::Catalog;
    let baseline = app.project.content_baseline();
    let sources = app.project.sources().clone();

    click(&ctx, &mut app, 14, "组合查询与待办");
    let output = frame(&ctx, &mut app, Vec::new(), 14);

    assert!(output
        .shapes
        .iter()
        .any(|shape| { text_position(&shape.shape, "筛选条件").is_some() }));
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.sources(), sources);
    assert!(app.history.is_empty());
}

#[test]
fn catalog_query_composes_core_filters_shows_reasons_and_removes_a_condition() {
    let (ctx, mut app) = app();
    app.tab = super::Tab::Catalog;
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 14, "组合查询与待办");
    click(&ctx, &mut app, 14, "＋ 添加条件");
    click(&ctx, &mut app, 14, "对象类型");
    click(&ctx, &mut app, 14, "实体 · entity");
    click(&ctx, &mut app, 14, "＋ 添加条件");
    click(&ctx, &mut app, 14, "名称 / ID / 别名");
    enter_text_at_placeholder_in_window(&ctx, &mut app, 14, "输入名称、ID 或别名", "同名");
    click(&ctx, &mut app, 14, "添加名称值");
    click(&ctx, &mut app, 14, "运行查询");
    let rendered = rendered_text_in_window(&ctx, &mut app, 14, "2 个命中");

    assert!(rendered.contains("2 个命中"), "{rendered}");
    assert!(rendered.contains("命中：类型"), "{rendered}");
    assert!(rendered.contains("命中：名称/别名"), "{rendered}");
    assert!(rendered.contains("名称/别名：「同名」"), "{rendered}");
    assert!(rendered.contains("实体 · 同名 · a"), "{rendered}");
    assert!(rendered.contains("实体 · 同名 · b"), "{rendered}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());

    click(&ctx, &mut app, 14, "删除名称 / ID / 别名条件");
    let after_remove = rendered_text_in_window(&ctx, &mut app, 14, "筛选条件");
    assert!(!after_remove.contains("2 个命中"), "{after_remove}");
    assert!(after_remove.contains("类型"), "{after_remove}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn catalog_query_reports_an_empty_result_without_changing_the_project() {
    let (ctx, mut app) = app();
    app.tab = super::Tab::Catalog;
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 14, "组合查询与待办");
    click(&ctx, &mut app, 14, "＋ 添加条件");
    click(&ctx, &mut app, 14, "名称 / ID / 别名");
    enter_text_at_placeholder_in_window(&ctx, &mut app, 14, "输入名称、ID 或别名", "不存在的资料");
    click(&ctx, &mut app, 14, "添加名称值");
    click(&ctx, &mut app, 14, "运行查询");
    let rendered = rendered_text_in_window(&ctx, &mut app, 14, "没有找到匹配资料");

    assert!(rendered.contains("没有找到匹配资料"), "{rendered}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn catalog_query_uses_core_pages_and_rejects_a_result_from_an_old_project_baseline() {
    let (ctx, mut app) = app();
    app.tab = super::Tab::Catalog;
    let entry = app.project.entry.clone();
    let mut source = app.project.sources()[&entry].clone();
    for index in 0..55 {
        source.push_str(&format!(
            "entity object_{index} kind place as \"对象 {index}\"\n"
        ));
    }
    app.project.set_text(&entry, source).unwrap();
    app.recompile();

    click(&ctx, &mut app, 14, "组合查询与待办");
    click(&ctx, &mut app, 14, "＋ 添加条件");
    click(&ctx, &mut app, 14, "对象类型");
    click(&ctx, &mut app, 14, "实体 · entity");
    click(&ctx, &mut app, 14, "运行查询");
    let first_page = rendered_text_in_window(&ctx, &mut app, 14, "57 个命中");
    assert!(first_page.contains("显示 1–50"), "{first_page}");
    assert!(first_page.contains("下一页"), "{first_page}");

    let mut changed_source = app.project.sources()[&entry].clone();
    changed_source.push_str("entity object_new kind place as \"新对象\"\n");
    app.project.set_text(&entry, changed_source).unwrap();
    app.recompile();
    let stale_page = rendered_text_in_window(&ctx, &mut app, 14, "结果已过期");
    assert!(stale_page.contains("结果已过期"), "{stale_page}");
    assert!(!stale_page.contains("下一页"), "{stale_page}");

    click(&ctx, &mut app, 14, "从第一页重新查询");
    let refreshed = rendered_text_in_window(&ctx, &mut app, 14, "58 个命中");
    assert!(refreshed.contains("58 个命中"), "{refreshed}");
    assert!(refreshed.contains("显示 1–50"), "{refreshed}");
}

#[test]
fn catalog_query_saves_shared_definitions_and_keeps_local_favorites_out_of_the_project() {
    let (ctx, mut app) = app();
    app.tab = super::Tab::Catalog;
    let root = app.project.root.clone();
    let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
    click(&ctx, &mut app, 14, "组合查询与待办");
    click(&ctx, &mut app, 14, "＋ 添加条件");
    click(&ctx, &mut app, 14, "对象类型");
    click(&ctx, &mut app, 14, "实体 · entity");
    click(&ctx, &mut app, 14, "保存为共享查询");
    enter_text_at_placeholder_in_window(&ctx, &mut app, 14, "稳定 ID", "people");
    enter_text_at_placeholder_in_window(&ctx, &mut app, 14, "查询名称", "所有资料");
    click(&ctx, &mut app, 14, "保存共享定义");

    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert!(app
        .project
        .saved_query_index()
        .queries
        .contains_key("people"));
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
    assert_eq!(app.history.len(), 1);
    click(&ctx, &mut app, 14, "共享查询定义 · 1 项");
    click(&ctx, &mut app, 14, "☆ 收藏到本机");
    let baseline = app.project.content_baseline();
    assert_eq!(app.history.len(), 1);

    app.project.save().unwrap();
    app.project = Project::open(&root).unwrap();
    app.reset_views();
    app.recompile();
    app.tab = super::Tab::Catalog;
    click(&ctx, &mut app, 14, "组合查询与待办");
    click(&ctx, &mut app, 14, "共享查询定义 · 1 项");
    let output = frame(&ctx, &mut app, Vec::new(), 14);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("所有资料 · people"), "{rendered}");
    click(&ctx, &mut app, 14, "本地收藏 · 1 项");
    let favorites = frame(&ctx, &mut app, Vec::new(), 14);
    let mut favorite_text = String::new();
    for shape in &favorites.shapes {
        collect_text(&shape.shape, &mut favorite_text);
    }
    assert!(
        favorite_text.contains("所有资料 · people"),
        "{favorite_text}"
    );
    if !favorite_text.contains("载入") {
        click(&ctx, &mut app, 14, "共享查询定义 · 1 项");
    }
    click(&ctx, &mut app, 14, "载入");
    assert_eq!(app.project.content_baseline(), baseline);
    click(&ctx, &mut app, 14, "运行查询");
    let reopened_query = rendered_text_in_window(&ctx, &mut app, 14, "2 个命中");
    assert!(reopened_query.contains("2 个命中"), "{reopened_query}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.history.len(), 0);
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
}

#[test]
fn catalog_todo_groups_core_items_and_jumps_to_the_exact_source_without_writing() {
    let (ctx, mut app) = app();
    app.tab = super::Tab::Catalog;
    let entry = app.project.entry.clone();
    app.project
        .set_text(
            &entry,
            concat!(
                "entity keepers kind organization as \"守灯会\"\n",
                "event start\n",
                "  参见 [[entity:missing_place|失落地点]] 与 [[entity:missing_place|另一地点]]。\n",
                "  -> END\n",
            )
            .into(),
        )
        .unwrap();
    let manifest = app.project.root.join(".world/project.json");
    app.project
        .set_authoring_document(
            &manifest,
            br#"{"schema_version":1,"language_version":"1.10","required_features":["content.entities.v1","collaboration.comments.v1","collaboration.proposals.v1"],"maps":{},"graph_views":{},"comments":{"detached":".world/comments/detached.json","resolved":".world/comments/resolved.json"},"proposals":{"open":".world/proposals/open.json","accepted":".world/proposals/accepted.json"}}"#.to_vec(),
        )
        .unwrap();
    app.project
        .create_authoring_document(
            &app.project.root.join(".world/comments/detached.json"),
            r#"{"schema_version":1,"id":"detached","author":"甲","body":"检查失落地点","anchor":{"kind":"object","target":{"kind":"entity","id":"missing_place"}},"resolved":false}"#.as_bytes().to_vec(),
        )
        .unwrap();
    app.project
        .create_authoring_document(
            &app.project.root.join(".world/comments/resolved.json"),
            r#"{"schema_version":1,"id":"resolved","author":"甲","body":"已处理","anchor":{"kind":"object","target":{"kind":"entity","id":"missing_place"}},"resolved":true}"#.as_bytes().to_vec(),
        )
        .unwrap();
    for (id, status) in [("open", "open"), ("accepted", "accepted")] {
        app.project
            .create_authoring_document(
                &app.project.root.join(format!(".world/proposals/{id}.json")),
                format!(
                    r#"{{"schema_version":1,"id":"{id}","author":"甲","reason":"审阅改动","status":"{status}","changes":[{{"path":"world.wl","domain":"content","base":"旧文本","proposed":"新文本"}}]}}"#
                )
                .into_bytes(),
            )
            .unwrap();
    }
    app.recompile();
    let baseline = app.project.content_baseline();
    let sources = app.project.sources();
    click(&ctx, &mut app, 14, "组合查询与待办");
    click(&ctx, &mut app, 14, "统一待办");
    let rendered = rendered_text_in_window(&ctx, &mut app, 14, "待审提案 · 1 项");

    for label in [
        "断链 · 2 项",
        "待建资料 · 1 项",
        "失锚批注 · 1 项",
        "待审提案 · 1 项",
    ] {
        assert!(rendered.contains(label), "missing {label}: {rendered}");
    }
    assert!(!rendered.contains("accepted"), "{rendered}");
    assert!(!rendered.contains("resolved"), "{rendered}");
    assert!(rendered.contains("world.wl:3"), "{rendered}");

    click(&ctx, &mut app, 14, "定位断链来源");
    assert_eq!(app.tab, super::Tab::Edit);
    assert_eq!(app.active_file, entry);
    let (line, column) = app.jump.unwrap();
    assert_eq!(line, 3);
    assert!(column > 0);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.sources(), sources);
    assert!(app.history.is_empty());
}

#[test]
fn pinning_two_reading_panels_keeps_source_and_undo_unchanged() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    app.open_reading(TargetRef::new("entity", "a"));
    click(&ctx, &mut app, 8, "钉住旁查");
    app.open_reading(TargetRef::new("entity", "b"));
    click(&ctx, &mut app, 8, "钉住旁查");
    assert_eq!(app.reading_panels.ids().len(), 2);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    app.reset_views();
    assert!(app.reading_panels.ids().is_empty());
}

#[test]
fn narrow_reading_panels_can_switch_and_close_without_losing_an_edit_draft() {
    let (ctx, mut app) = app();
    let first = app
        .reading_panels
        .pin(TargetRef::new("entity", "a"))
        .unwrap();
    let second = app
        .reading_panels
        .pin(TargetRef::new("entity", "b"))
        .unwrap();
    app.selected_reading_panel = Some(first);
    app.edit_entity(Some("a"));
    app.entity_editor.as_mut().unwrap().draft.description = "未提交资料".into();
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 9, "旁查 2");
    assert_eq!(app.selected_reading_panel, Some(second));
    click(&ctx, &mut app, 9, "返回源码编辑");
    assert_eq!(app.tab, super::Tab::Edit);
    click(&ctx, &mut app, 9, "编辑此对象");
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.description,
        "未提交资料"
    );
    click(&ctx, &mut app, 9, "关闭旁查");
    assert!(app.reading_panels.get(second).is_none());
    assert!(app.reading_panels.get(first).is_some());
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.description,
        "未提交资料"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn project_switch_preserves_unsubmitted_form_even_with_clean_project() {
    let (ctx, mut app) = app();
    app.project.mark_saved();
    app.edit_entity(Some("a"));
    app.entity_editor.as_mut().unwrap().draft.description = "未提交资料".into();
    app.request_action(super::Pending::Close, &ctx);
    assert!(!app.allow_close);
    assert!(app.pending.is_none());
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.description,
        "未提交资料"
    );
}

#[test]
fn project_switch_preserves_an_unsubmitted_period_form() {
    let (ctx, mut app) = app();
    app.project.mark_saved();
    app.new_period = Some(("age".into(), "未提交时代".into(), None));
    app.request_action(super::Pending::Close, &ctx);
    assert!(!app.allow_close);
    assert!(app.pending.is_none());
    assert_eq!(app.new_period.as_ref().unwrap().1, "未提交时代");
}

#[test]
fn pinned_wiki_navigation_does_not_close_an_independent_temporary_reader() {
    let (ctx, mut app) = app();
    let id = app
        .reading_panels
        .pin(TargetRef::new("entity", "a"))
        .unwrap();
    app.open_reading(TargetRef::new("entity", "b"));
    app.active_reading_panel = Some(id);
    click(&ctx, &mut app, 10, "在 Wiki 中查看");
    assert_eq!(app.reading_target, Some(TargetRef::new("entity", "b")));
    assert_eq!(app.wiki_target, Some(TargetRef::new("entity", "a")));
    let hit = &app
        .snapshot
        .as_ref()
        .unwrap()
        .wiki
        .occurrences(&TargetRef::new("entity", "a"))[0];
    let source_button = format!(
        "{}:{}:{}",
        hit.file.strip_prefix(&app.project.root).unwrap().display(),
        hit.line,
        hit.column
    );
    click(&ctx, &mut app, 10, &source_button);
    assert_eq!(app.reading_target, Some(TargetRef::new("entity", "b")));
}

#[test]
fn pinned_deleted_target_shows_invalid_identity_instead_of_another_object() {
    let (ctx, mut app) = app();
    let id = app
        .reading_panels
        .pin(TargetRef::new("entity", "a"))
        .unwrap();
    app.project
        .set_text(
            &app.project.entry.clone(),
            "entity b kind place as \"同名\"\n".into(),
        )
        .unwrap();
    app.recompile();
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 9);
    }
    let output = frame(&ctx, &mut app, Vec::new(), 9);
    assert!(output.shapes.iter().any(|shape| text_position(
        &shape.shape,
        "资料已失效：entity:a。可能已被删除或更改 ID。"
    )
    .is_some()));
    assert_eq!(app.reading_panels.get(id).unwrap().target.id, "a");
}
fn text_position(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text == label => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| text_position(shape, label)),
        _ => None,
    }
}
fn source_text_position(shape: &egui::Shape, source: &str, needle: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text == source => {
            let byte = source.find(needle)?;
            let cursor = source[..byte].chars().count() + needle.chars().count() / 2;
            Some(
                text.pos
                    + text
                        .galley
                        .pos_from_cursor(egui::text::CCursor::new(cursor))
                        .center()
                        .to_vec2(),
            )
        }
        egui::Shape::Vec(shapes) => shapes
            .iter()
            .find_map(|shape| source_text_position(shape, source, needle)),
        _ => None,
    }
}
fn click(ctx: &egui::Context, app: &mut WorldeditApp, window: u8, label: &str) {
    for _ in 0..3 {
        let _ = frame(ctx, app, Vec::new(), window);
    }
    let output = frame(ctx, app, Vec::new(), window);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position(&shape.shape, label))
        .unwrap_or_else(|| panic!("按钮未显示：{label}"));
    for pressed in [true, false] {
        let _ = frame(
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
            window,
        );
    }
}

fn open_selected_entity_form(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    source: &str,
    selected: &str,
) -> std::path::PathBuf {
    let entry = app.project.entry.clone();
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;
    let byte_start = source.find(selected).unwrap();
    let start = source[..byte_start].chars().count();
    let end = start + selected.chars().count();
    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(end),
        )));
    egui::TextEdit::store_state(ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let _ = frame(ctx, app, Vec::new(), 11);
    click(ctx, app, 11, "从选中文本建档");
    entry
}

#[test]
fn source_mention_lists_ambiguous_targets_and_explicit_choice_inserts_one_stable_link() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let source = "entity a kind place as \"同名\"\nentity b kind organization as \"同名\"\nevent start\n  开始：";
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;

    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(source.chars().count()),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let _ = frame(&ctx, &mut app, vec![Event::Text("@同名".into())], 11);

    let output = frame(&ctx, &mut app, Vec::new(), 11);
    assert!(
        output
            .shapes
            .iter()
            .any(|shape| text_position(&shape.shape, "实体 · 同名 · entity:a").is_some()),
        "候选必须显示 kind 和 ID，避免同名时静默选择"
    );
    assert!(output.shapes.iter().any(|shape| text_position(
        &shape.shape,
        "实体 · 同名 · entity:b"
    )
    .is_some()));

    let before_commit = app.project.content_baseline();
    let history_before = app.history.len();
    click(&ctx, &mut app, 11, "实体 · 同名 · entity:b");
    assert!(
        app.project
            .document(&entry)
            .unwrap()
            .contains("[[entity:b|同名]]"),
        "source={:?}, error={:?}",
        app.project.document(&entry).unwrap(),
        app.io_error
    );
    let links = &app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .text_links;
    assert!(links.iter().any(|link| {
        link.target == worldline_core::TargetRef::new("entity", "b") && link.label == "同名"
    }));
    assert_eq!(app.history.len(), history_before + 1);
    app.undo(false);
    assert_eq!(app.project.content_baseline(), before_commit);
}

#[test]
fn source_mention_candidates_can_be_selected_and_committed_without_a_mouse() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let source = "entity a kind place as \"同名\"\nentity b kind organization as \"同名\"\nevent start\n  开始：";
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;

    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(source.chars().count()),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let _ = frame(&ctx, &mut app, vec![Event::Text("@同名".into())], 11);
    let candidates = frame(&ctx, &mut app, Vec::new(), 11);
    assert!(candidates.shapes.iter().any(|shape| text_position(
        &shape.shape,
        "实体 · 同名 · entity:a"
    )
    .is_some()));

    let key = |key| Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = frame(&ctx, &mut app, vec![key(egui::Key::ArrowDown)], 11);
    let _ = frame(&ctx, &mut app, vec![key(egui::Key::Enter)], 11);

    assert!(app
        .project
        .document(&entry)
        .unwrap()
        .contains("[[entity:b|同名]]"));
    assert_eq!(app.history.len(), 2);
}

#[test]
fn source_mention_esc_hides_candidates_without_changing_source() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let source = "entity a kind place as \"同名\"\nevent start\n  开始：";
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;
    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(source.chars().count()),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let _ = frame(&ctx, &mut app, vec![Event::Text("@同名".into())], 11);
    let shown = frame(&ctx, &mut app, Vec::new(), 11);
    assert!(shown
        .shapes
        .iter()
        .any(|shape| { text_position(&shape.shape, "实体 · 同名 · entity:a").is_some() }));

    let baseline = app.project.content_baseline();
    let output = frame(
        &ctx,
        &mut app,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: Some(egui::Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        11,
    );
    assert!(!output
        .shapes
        .iter()
        .any(|shape| { text_position(&shape.shape, "实体 · 同名 · entity:a").is_some() }));
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.project.document(&entry).unwrap().contains("@同名"));
}

#[test]
fn source_mention_waits_for_ime_commit_before_showing_candidates() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let source = "entity a kind place as \"同名\"\nevent start\n  开始：";
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;
    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(source.chars().count()),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let _ = frame(
        &ctx,
        &mut app,
        vec![
            Event::Ime(egui::ImeEvent::Enabled),
            Event::Ime(egui::ImeEvent::Preedit("@同名".into())),
        ],
        11,
    );
    let preedit = frame(&ctx, &mut app, Vec::new(), 11);
    assert!(!preedit
        .shapes
        .iter()
        .any(|shape| { text_position(&shape.shape, "实体 · 同名 · entity:a").is_some() }));
    assert!(preedit.shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.contains("@同名"))
    }));
    assert_eq!(app.project.document(&entry).unwrap(), source);

    let _ = frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Commit("@同名".into()))],
        11,
    );
    let committed = frame(&ctx, &mut app, Vec::new(), 11);
    assert!(committed
        .shapes
        .iter()
        .any(|shape| { text_position(&shape.shape, "实体 · 同名 · entity:a").is_some() }));
    assert_eq!(app.history.len(), 1);
    assert!(app.project.document(&entry).unwrap().contains("@同名"));
}

#[test]
fn ime_commit_after_external_refresh_keeps_external_source_and_preserves_local_draft() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let source = "entity a kind place as \"同名\"\nevent start\n  原始正文";
    app.project.set_text(&entry, source.into()).unwrap();
    app.project.save().unwrap();
    app.tab = super::Tab::Edit;

    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(source.chars().count()),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let _ = frame(
        &ctx,
        &mut app,
        vec![
            Event::Ime(egui::ImeEvent::Enabled),
            Event::Ime(egui::ImeEvent::Preedit("@同名".into())),
        ],
        11,
    );
    assert!(app.ime_source_draft.is_some());
    assert_eq!(app.project.document(&entry).unwrap(), source);

    let external = format!("{source}\n外部版本");
    std::fs::write(&entry, external.as_bytes()).unwrap();
    assert!(app.project.refresh().unwrap().is_empty());
    app.recompile();
    let external_baseline = app.project.content_baseline();

    let _ = frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Commit("@同名".into()))],
        11,
    );

    assert_eq!(app.project.content_baseline(), external_baseline);
    assert_eq!(app.project.document(&entry).unwrap(), external);
    let (draft_path, draft, _) = app.ime_source_draft.as_ref().unwrap();
    assert_eq!(draft_path, &entry);
    assert!(
        draft.contains("@同名"),
        "local IME input must remain recoverable"
    );
    assert!(app
        .io_error
        .as_deref()
        .is_some_and(|error| error.contains("外部修改")));
    assert_eq!(std::fs::read_to_string(&entry).unwrap(), external);

    click(&ctx, &mut app, 11, "放弃本地草稿并恢复外部版本");
    assert!(app.ime_source_draft.is_none());
    assert_eq!(app.project.document(&entry).unwrap(), external);
    assert_eq!(std::fs::read_to_string(&entry).unwrap(), external);
}

#[test]
fn selected_source_text_can_create_a_linked_entity_and_one_undo_restores_raw_text() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let source = "entity a kind place as \"同名\"\nevent start\n  发现失落城池，随后找到遗迹";
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;
    let selected = "失落城池";
    let byte_start = source.find(selected).unwrap();
    let start = source[..byte_start].chars().count();
    let end = start + selected.chars().count();
    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(end),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let baseline = app.project.content_baseline();
    let _ = frame(&ctx, &mut app, Vec::new(), 11);
    click(&ctx, &mut app, 11, "从选中文本建档");
    assert_eq!(app.project.document(&entry).unwrap(), source);
    assert_eq!(app.project.content_baseline(), baseline);
    let form = app.entity_editor.as_ref().unwrap();
    let id = form.draft.id.clone();
    assert_eq!(form.draft.display, selected);
    assert_eq!(
        form.source_selection.as_ref().unwrap().expected_text,
        selected
    );

    click(&ctx, &mut app, 0, "应用资料");
    assert!(app.entity_editor.is_none(), "{:?}", app.io_error);
    assert!(app
        .project
        .document(&entry)
        .unwrap()
        .contains(&format!("[[entity:{id}|{selected}]]")));
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities
        .contains_key(&id));
    assert_eq!(app.history.len(), 1);
    app.undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&entry).unwrap(), source);
}

#[test]
fn selected_source_text_can_open_and_apply_the_entity_form_without_a_mouse() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let source = "entity a kind place as \"同名\"\nevent start\n  发现失落城池，随后找到遗迹";
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;
    let selected = "失落城池";
    let byte_start = source.find(selected).unwrap();
    let start = source[..byte_start].chars().count();
    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(start),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let shift_right = Event::Key {
        key: egui::Key::ArrowRight,
        physical_key: Some(egui::Key::ArrowRight),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::SHIFT,
    };
    let _ = frame(&ctx, &mut app, Vec::new(), 11);
    assert!(ctx.memory(|memory| memory.has_focus(editor)));
    let _ = frame(
        &ctx,
        &mut app,
        vec![shift_right; selected.chars().count()],
        11,
    );
    let range = egui::TextEdit::load_state(&ctx, editor)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!(
        range.primary.index.abs_diff(range.secondary.index),
        selected.chars().count()
    );
    assert!(ctx.memory(|memory| memory.has_focus(editor)));
    assert_eq!(
        source
            .chars()
            .skip(range.primary.index.min(range.secondary.index))
            .take(range.primary.index.abs_diff(range.secondary.index))
            .collect::<String>(),
        selected
    );
    let shortcut = Event::Key {
        key: egui::Key::Enter,
        physical_key: Some(egui::Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    };

    let _ = frame(&ctx, &mut app, vec![shortcut.clone()], 11);
    assert!(app.entity_editor.is_some());
    assert_eq!(app.project.document(&entry).unwrap(), source);
    assert_eq!(app.entity_editor.as_ref().unwrap().draft.display, selected);

    let id = app.entity_editor.as_ref().unwrap().draft.id.clone();
    let _ = frame(&ctx, &mut app, Vec::new(), 0);
    let _ = frame(&ctx, &mut app, vec![shortcut], 0);
    assert!(app.entity_editor.is_none());
    assert!(app
        .project
        .document(&entry)
        .unwrap()
        .contains(&format!("[[entity:{id}|{selected}]]")));
    assert_eq!(app.history.len(), 1);
}

#[test]
fn clicking_a_source_link_opens_reading_and_returns_to_the_same_editor_cursor() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let link = "[[entity:a|同名]]";
    let source = format!("entity a kind place as \"同名\"\nevent start\n  看见{link}，继续");
    app.project.set_text(&entry, source.clone()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 11);
    }
    let output = frame(&ctx, &mut app, Vec::new(), 11);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| source_text_position(&shape.shape, &source, link))
        .expect("源码链接必须在编辑器中可定位");
    for pressed in [true, false] {
        let _ = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            11,
        );
    }
    assert_eq!(app.reading_target, Some(TargetRef::new("entity", "a")));
    let (return_path, return_cursor) = app.reading_return.clone().unwrap();
    assert_eq!(return_path, entry);

    click(&ctx, &mut app, 8, "返回源码编辑");
    assert!(app.reading_target.is_none());
    assert_eq!(app.tab, super::Tab::Edit);
    assert_eq!(app.active_file, entry);
    assert_eq!(
        egui::TextEdit::load_state(&ctx, egui::Id::new(("source", &entry)))
            .unwrap()
            .cursor
            .char_range()
            .unwrap()
            .primary
            .index,
        return_cursor
    );
    assert!(app.reading_return.is_none());
}

#[test]
fn source_links_can_be_opened_and_returned_to_by_keyboard() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let link = "[[entity:a|同名]]";
    let source = format!("entity a kind place as \"同名\"\nevent start\n  看见{link}，继续");
    app.project.set_text(&entry, source.clone()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;
    let byte = source.find(link).unwrap();
    let cursor = source[..byte].chars().count() + 5;
    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(cursor),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let shortcut = Event::Key {
        key: egui::Key::Enter,
        physical_key: Some(egui::Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    };

    let _ = frame(&ctx, &mut app, vec![shortcut], 11);
    assert_eq!(app.reading_target, Some(TargetRef::new("entity", "a")));
    let (_, return_cursor) = app.reading_return.clone().unwrap();
    let _ = frame(
        &ctx,
        &mut app,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: Some(egui::Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        8,
    );
    assert!(app.reading_target.is_none());
    assert_eq!(app.tab, super::Tab::Edit);
    assert_eq!(
        egui::TextEdit::load_state(&ctx, editor)
            .unwrap()
            .cursor
            .char_range()
            .unwrap()
            .primary
            .index,
        return_cursor
    );
}

#[test]
fn cancelling_a_selected_text_draft_preserves_the_unmarked_source_and_project() {
    let (ctx, mut app) = app();
    let source = "entity a kind place as \"同名\"\nevent start\n  发现失落城池，随后找到遗迹";
    let entry = open_selected_entity_form(&ctx, &mut app, source, "失落城池");
    let baseline = app.project.content_baseline();
    app.entity_editor.as_mut().unwrap().draft.description = "尚未提交".into();
    click(&ctx, &mut app, 0, "取消");
    assert!(app.entity_editor.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&entry).unwrap(), source);
    assert!(app.history.is_empty());
}

#[test]
fn escape_closes_a_selected_text_draft_without_mutating_the_source() {
    let (ctx, mut app) = app();
    let source = "entity a kind place as \"同名\"\nevent start\n  发现失落城池，随后找到遗迹";
    let entry = open_selected_entity_form(&ctx, &mut app, source, "失落城池");
    let baseline = app.project.content_baseline();
    app.entity_editor.as_mut().unwrap().draft.display = "暂存名称".into();
    let _ = frame(
        &ctx,
        &mut app,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: Some(egui::Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        0,
    );
    assert!(app.entity_editor.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&entry).unwrap(), source);
    assert!(app.history.is_empty());
}

#[test]
fn external_source_change_rejects_selected_entity_apply_and_keeps_the_draft() {
    let (ctx, mut app) = app();
    let source = "entity a kind place as \"同名\"\nevent start\n  发现失落城池，随后找到遗迹";
    let entry = open_selected_entity_form(&ctx, &mut app, source, "失落城池");
    let baseline = app.project.content_baseline();
    let id = app.entity_editor.as_ref().unwrap().draft.id.clone();
    app.entity_editor.as_mut().unwrap().draft.description = "保留在草稿中".into();
    std::fs::create_dir_all(&app.project.root).unwrap();
    std::fs::write(&entry, b"external source version").unwrap();

    click(&ctx, &mut app, 0, "应用资料");
    assert!(app.entity_editor.is_some());
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.description,
        "保留在草稿中"
    );
    assert!(app
        .io_error
        .as_deref()
        .is_some_and(|error| error.contains("外部修改")));
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(!app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities
        .contains_key(&id));
    std::fs::remove_file(entry).unwrap();
}

#[test]
fn missing_entity_link_offers_a_repair_draft_with_the_stable_missing_id() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    app.project
        .set_text(
            &entry,
            "entity a kind place as \"同名\"\nevent start\n  参考[[entity:missing_place|失落城池]]。"
                .into(),
        )
        .unwrap();
    app.recompile();
    let target = TargetRef::new("entity", "missing_place");
    app.open_reading(target.clone());
    let output = frame(&ctx, &mut app, Vec::new(), 12);
    assert!(output.shapes.iter().any(|shape| {
        text_position(
            &shape.shape,
            "资料已失效：entity:missing_place。可能已被删除或更改 ID。",
        )
        .is_some()
    }));
    assert!(output.shapes.iter().any(|shape| {
        text_position(&shape.shape, "按此 ID 新建实体以修复引用").is_some()
    }));
    click(&ctx, &mut app, 12, "按此 ID 新建实体以修复引用");
    let form = app.entity_editor.as_ref().unwrap();
    assert_eq!(form.draft.id, "missing_place");
    assert_eq!(form.draft.display, "失落城池");
    click(&ctx, &mut app, 0, "应用资料");
    assert!(app.entity_editor.is_none(), "{:?}", app.io_error);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .object(&target)
        .is_some());
    assert!(!app
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == "A218"));
}

#[test]
fn manuscript_reordering_uses_one_core_command_and_undo_keeps_story_semantics() {
    let (ctx, mut app) = manuscript_app();
    let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
    let content_baseline = app.project.content_baseline();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());

    click(&ctx, &mut app, 13, "下移");
    assert_eq!(
        app.project
            .manuscript_index("novel")
            .unwrap()
            .page(0, 10)
            .chapters[0]
            .id,
        "opening"
    );
    assert!(app.history.is_empty(), "草稿重排尚未应用");
    click(&ctx, &mut app, 13, "应用书稿");

    let reordered = app.project.manuscript_index("novel").unwrap();
    assert_eq!(reordered.page(0, 10).chapters[0].id, "departure");
    assert_eq!(app.history.len(), 1);
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
    assert_eq!(
        app.snapshot
            .as_ref()
            .unwrap()
            .result
            .program
            .events
            .iter()
            .map(|event| event.name.as_str())
            .collect::<Vec<_>>(),
        ["arrival", "departure"]
    );

    app.undo(false);
    assert_eq!(
        app.project
            .manuscript_index("novel")
            .unwrap()
            .page(0, 10)
            .chapters[0]
            .id,
        "opening"
    );
    assert_eq!(app.project.content_baseline(), content_baseline);
}

#[test]
fn manuscript_preview_and_stats_come_from_core_projection() {
    let (ctx, mut app) = manuscript_app();
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("汉字 8 · 词数 8"), "{rendered}");
    assert!(rendered.contains("甲乙"), "{rendered}");
    assert!(rendered.contains("灯塔亮起"), "{rendered}");
    assert!(
        !rendered.contains("-> END"),
        "控制流不能混入阅读正文：{rendered}"
    );
    assert!(rendered.contains("书稿只决定阅读顺序"), "{rendered}");
    assert!(
        rendered.contains("独立于世界时间与事件控制流"),
        "{rendered}"
    );
    click(&ctx, &mut app, 13, "卡片");
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let mut cards = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut cards);
    }
    assert!(cards.contains("查看章节"), "{cards}");
    click(&ctx, &mut app, 13, "列表");
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let mut list = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut list);
    }
    assert!(list.contains("抵达"), "{list}");
    click(&ctx, &mut app, 13, "章节树");
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position_contains(&shape.shape, "灯塔亮起"))
        .unwrap();
    for _ in 0..8 {
        let _ = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(point),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: vec2(0.0, -12.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            13,
        );
    }
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let mut scrolled = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut scrolled);
    }
    assert!(
        scrolled.contains("同名地点资料"),
        "书稿长预览应可滚动：{scrolled}"
    );
}

#[test]
fn manuscript_body_editor_keeps_unsubmitted_input_and_applies_through_egui() {
    let (ctx, mut app) = manuscript_app();
    let baseline = app.project.content_baseline();
    let replacement = concat!(
        "character traveler as \"旅人\"\n",
        "event arrival as \"抵达\"\n",
        "  全新正文。\n",
        "  -> END\n",
        "event departure as \"离港\"\n",
        "  远航。\n",
        "  -> END\n",
    );
    replace_manuscript_source(&ctx, &mut app, replacement);
    assert_eq!(app.project.content_baseline(), baseline, "输入仍只是草稿");
    click(&ctx, &mut app, 13, "应用正文草稿");
    assert!(app
        .project
        .document(&app.active_file)
        .unwrap()
        .contains("全新正文。"));
    assert_eq!(app.history.len(), 1);
    app.undo(false);
    assert!(app
        .project
        .document(&app.active_file)
        .unwrap()
        .contains("甲乙"));
}

#[test]
fn stale_manuscript_body_keeps_input_and_does_not_overwrite_new_source() {
    let (ctx, mut app) = manuscript_app();
    let replacement = concat!(
        "character traveler as \"旅人\"\n",
        "event arrival as \"抵达\"\n",
        "  保留在草稿的文字。\n",
        "  -> END\n",
    );
    replace_manuscript_source(&ctx, &mut app, replacement);
    let externally_changed = "event arrival as \"抵达\"\n  外部修改。\n  -> END\n";
    app.project
        .set_text(&app.active_file.clone(), externally_changed.into())
        .unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();

    click(&ctx, &mut app, 13, "应用正文草稿");
    assert_eq!(
        app.project.document(&app.active_file).unwrap(),
        externally_changed
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    assert!(app
        .io_error
        .as_deref()
        .is_some_and(|error| error.contains("过期")));
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    assert!(
        output
            .shapes
            .iter()
            .any(|shape| text_position_contains(&shape.shape, "保留在草稿的文字").is_some()),
        "过期时保留编辑器输入"
    );
}

#[test]
fn manuscript_target_picker_disambiguates_same_display_names() {
    let (ctx, mut app) = manuscript_app();
    click(&ctx, &mut app, 13, "event:arrival");
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("同名 · entity:a"), "{rendered}");
    assert!(rendered.contains("同名 · entity:b"), "{rendered}");
}

#[test]
fn manuscript_missing_reference_can_be_repaired_through_candidate_picker() {
    let (ctx, mut app) = manuscript_app();
    let path = app.project.root.join(".world/manuscripts/novel.json");
    let mut book: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&path).unwrap().bytes()).unwrap();
    book["entries"][0]["target_ref"]["id"] = "deleted_event".into();
    app.project
        .set_authoring_document(&path, serde_json::to_vec(&book).unwrap())
        .unwrap();
    app.recompile();

    click(&ctx, &mut app, 13, "event:deleted_event");
    click(&ctx, &mut app, 13, "抵达 · event:arrival");
    click(&ctx, &mut app, 13, "应用书稿");

    let chapter = &app
        .project
        .manuscript_index("novel")
        .unwrap()
        .page(0, 10)
        .chapters[0];
    assert_eq!(chapter.target_ref.as_ref().unwrap().id, "arrival");
    assert_eq!(
        chapter.source.as_ref().unwrap().status,
        worldline_core::ManuscriptReferenceStatus::Resolved
    );
    assert_eq!(app.history.len(), 1);
}

#[test]
fn manuscript_creation_insert_save_reopen_and_full_reader_preview_use_core() {
    let (ctx, mut app) = app();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("worldedit-manuscript-e2e-{unique}"));
    app.project = Project::new(&root);
    app.active_file = app.project.entry.clone();
    let entry = app.project.entry.clone();
    app.project.documents.retain(|path, _| path == &entry);
    app.project
        .set_text(
            &app.active_file.clone(),
            "event opening as \"开篇\"\n  海雾散开。\n  -> END\n".into(),
        )
        .unwrap();
    app.project
        .create_authoring_document(
            &root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.10","entry":"world.wl","required_features":["presentation.manuscripts.v1"],"maps":{},"graph_views":{},"manuscripts":{}}"#.to_vec(),
        )
        .unwrap();
    app.reset_views();
    app.recompile();

    enter_text_at_placeholder(&ctx, &mut app, "例如 novel", "novel");
    enter_text_at_placeholder(&ctx, &mut app, "书稿名称", "雾港序章");
    click(&ctx, &mut app, 13, "创建并打开书稿");
    assert!(app.project.manuscript_indices().contains_key("novel"));
    assert_eq!(app.history.len(), 1);

    click(&ctx, &mut app, 13, "插入分节");
    click(&ctx, &mut app, 13, "插入章节");
    click(&ctx, &mut app, 13, "应用书稿");
    let chapter = &app
        .project
        .manuscript_index("novel")
        .unwrap()
        .page(0, 10)
        .chapters[0];
    assert_eq!(chapter.target_ref.as_ref().unwrap().id, "opening");
    assert_eq!(chapter.section_path, ["section"]);
    assert_eq!(
        chapter.source.as_ref().unwrap().status,
        worldline_core::ManuscriptReferenceStatus::Resolved
    );
    assert_eq!(app.history.len(), 2);

    app.project.save().unwrap();
    app.project = Project::open(&root).unwrap();
    app.reset_views();
    app.recompile();
    let reopened = app.project.manuscript_index("novel").unwrap();
    assert_eq!(reopened.title.as_deref(), Some("雾港序章"));
    assert_eq!(reopened.page(0, 10).chapters[0].id, "chapter");
    assert_eq!(reopened.page(0, 10).chapters[0].section_path, ["section"]);
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("海雾散开"), "{rendered}");
}

fn enter_text_at_placeholder(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    placeholder: &str,
    text: &str,
) {
    enter_text_at_placeholder_in_window(ctx, app, 13, placeholder, text);
}

fn enter_text_at_placeholder_in_window(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    placeholder: &str,
    text: &str,
) {
    for _ in 0..3 {
        let _ = frame(ctx, app, Vec::new(), window);
    }
    let output = frame(ctx, app, Vec::new(), window);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position_contains(&shape.shape, placeholder))
        .unwrap_or_else(|| panic!("未显示输入提示：{placeholder}"));
    for pressed in [true, false] {
        let _ = frame(
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
            window,
        );
    }
    let _ = frame(ctx, app, vec![Event::Text(text.into())], window);
}

fn rendered_text_in_window(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    needle: &str,
) -> String {
    let mut rendered = String::new();
    for _ in 0..80 {
        let output = frame(ctx, app, Vec::new(), window);
        rendered.clear();
        for shape in &output.shapes {
            collect_text(&shape.shape, &mut rendered);
        }
        if rendered.contains(needle) {
            break;
        }
    }
    rendered
}

fn replace_manuscript_source(ctx: &egui::Context, app: &mut WorldeditApp, replacement: &str) {
    click(ctx, app, 13, "编辑来源文件");
    for _ in 0..3 {
        let _ = frame(ctx, app, Vec::new(), 13);
    }
    let output = frame(ctx, app, Vec::new(), 13);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position_contains(&shape.shape, "event arrival as"))
        .expect("源码编辑器应显示原始事件声明");
    for pressed in [true, false] {
        let _ = frame(
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
            13,
        );
    }
    let _ = frame(
        ctx,
        app,
        vec![
            Event::Key {
                key: egui::Key::A,
                physical_key: Some(egui::Key::A),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
            Event::Key {
                key: egui::Key::A,
                physical_key: Some(egui::Key::A),
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
            Event::Text(replacement.into()),
        ],
        13,
    );
}

fn text_position_contains(shape: &egui::Shape, fragment: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text.contains(fragment) => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes
            .iter()
            .find_map(|shape| text_position_contains(shape, fragment)),
        _ => None,
    }
}

fn collect_text(shape: &egui::Shape, text: &mut String) {
    match shape {
        egui::Shape::Text(shape) => text.push_str(&shape.galley.job.text),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                collect_text(shape, text);
            }
        }
        _ => {}
    }
}
#[test]
fn entity_apply_is_one_real_ui_command_with_undo_and_redo() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    app.edit_entity(None);
    let form = app.entity_editor.as_mut().unwrap();
    let id = form.draft.id.clone();
    form.draft.display = "点击创建的地点".into();
    form.draft.description = "多行资料\n不是地图占位".into();
    click(&ctx, &mut app, 0, "应用资料");
    assert!(app.entity_editor.is_none(), "{:?}", app.io_error);
    assert_eq!(
        app.snapshot
            .as_ref()
            .unwrap()
            .result
            .analysis
            .catalog
            .entities[&id]
            .display,
        "点击创建的地点"
    );
    assert_eq!(app.history.len(), 1);
    app.undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.redo.len() == 1);
    app.undo(true);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities
        .contains_key(&id));
}
#[test]
fn stale_entity_apply_button_is_disabled_and_keeps_draft() {
    let (ctx, mut app) = app();
    app.edit_entity(Some("a"));
    app.entity_editor.as_mut().unwrap().draft.display = "尚未合并的输入".into();
    app.recompile();
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 0, "应用资料");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.display,
        "尚未合并的输入"
    );
}
#[test]
fn explicit_relation_and_type_apply_use_ui_buttons_and_keep_identity() {
    let (ctx, mut app) = app();
    app.edit_relation_type(None);
    let form = app.relation_type_editor.as_mut().unwrap();
    form.draft.display = "维护".into();
    let kind_id = form.draft.id.clone();
    click(&ctx, &mut app, 2, "应用关系类型");
    assert!(app.relation_type_editor.is_none(), "{:?}", app.io_error);
    app.edit_relation(None, Some(TargetRef::new("entity", "a")));
    let form = app.relation_editor.as_mut().unwrap();
    form.draft.relation_type = kind_id;
    form.draft.to = TargetRef::new("entity", "b");
    form.draft.source_note = Some("显式作者来源".into());
    let relation_id = form.draft.id.clone();
    click(&ctx, &mut app, 1, "应用独立关系");
    assert!(app.relation_editor.is_none(), "{:?}", app.io_error);
    let relation = &app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .relations[&relation_id];
    assert_eq!(relation.from_ref, TargetRef::new("entity", "a"));
    assert_eq!(relation.to_ref, TargetRef::new("entity", "b"));
    assert_eq!(relation.source_note.as_deref(), Some("显式作者来源"));
    assert_eq!(app.history.len(), 2);
}
#[test]
fn deletion_checkbox_is_required_by_the_actual_button() {
    let (ctx, mut app) = app();
    app.plan_content_deletion(TargetRef::new("entity", "a"));
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 3, "确认删除内容");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    click(&ctx, &mut app, 3, "我确认删除此内容，而不是仅隐藏显示");
    click(&ctx, &mut app, 3, "确认删除内容");
    assert!(app.delete_form.is_none(), "{:?}", app.io_error);
    assert!(!app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities
        .contains_key("a"));
    app.undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn network_browsing_is_personal_until_shared_layout_is_saved() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let mut source = app.project.sources()[&entry].clone();
    source.push_str("relation_def r type knows from entity a to entity b\n");
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    app.open_network(TargetRef::new("entity", "a"));
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 4);
    }
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    assert_eq!(app.network_state.result.as_ref().unwrap().edges.len(), 1);

    app.network_view_id = "view_a".into();
    app.network_view_title = "A 的关联".into();
    click(&ctx, &mut app, 4, "保存共享布局");
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), 1);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .graph_index
        .views
        .contains_key("view_a"));
    assert_ne!(app.project.content_baseline(), baseline);
}

#[test]
fn presentation_preset_save_is_explicit_and_apply_is_personal() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let mut source = app.project.sources()[&entry].clone();
    source.push_str("relation_def r type knows from entity a to entity b\n");
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    app.open_network(TargetRef::new("entity", "a"));
    app.network_view_id = "view_a".into();
    app.network_view_title = "A 的关联".into();
    click(&ctx, &mut app, 4, "保存共享布局");

    let before_preset = app.project.content_baseline();
    let history_before = app.history.len();
    app.open_preset_editor(None);
    {
        let form = app.preset_editor.as_mut().unwrap();
        form.draft.id = "preset_a".into();
        form.draft.title = "A 的专题".into();
        form.draft.map_id = None;
        form.draft.graph_view_id = Some("view_a".into());
    }
    click(&ctx, &mut app, 6, "保存展示预设");
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), history_before + 1);
    assert_ne!(app.project.content_baseline(), before_preset);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .preset_index
        .presets
        .contains_key("preset_a"));

    let saved = app.project.content_baseline();
    let history_after_save = app.history.len();
    app.apply_presentation_preset("preset_a");
    assert_eq!(app.project.content_baseline(), saved);
    assert_eq!(app.history.len(), history_after_save);
    assert_eq!(app.network_state.focus, Some(TargetRef::new("entity", "a")));
}

#[cfg(not(debug_assertions))]
#[test]
fn network_release_profile_meets_m2_frame_and_reading_gates() {
    let (ctx, mut app) = app();
    let mut source = String::from("relation_type links as \"连接\"\n");
    for index in 0..1000 {
        source.push_str(&format!("entity e{index} kind place as \"对象 {index}\"\n"));
    }
    for index in 0..3000 {
        let from = if index < 600 { 0 } else { index % 1000 };
        let to = (index * 37 + 1) % 1000;
        source.push_str(&format!(
            "relation_def r{index} type links from entity e{from} to entity e{to}\n"
        ));
    }
    let entry = app.project.entry.clone();
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    app.open_network(TargetRef::new("entity", "e0"));

    for _ in 0..8 {
        let _ = frame(&ctx, &mut app, Vec::new(), 4);
    }
    let mut frame_ms = Vec::new();
    for _ in 0..160 {
        let started = std::time::Instant::now();
        let _ = frame(&ctx, &mut app, Vec::new(), 4);
        frame_ms.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    frame_ms.sort_by(f64::total_cmp);
    let p95_frame = frame_ms[(frame_ms.len() * 95 / 100).min(frame_ms.len() - 1)];
    let max_frame = *frame_ms.last().unwrap();

    let mut reading_ms = Vec::new();
    for index in 0..160 {
        app.open_reading(TargetRef::new("entity", &format!("e{}", index % 1000)));
        let started = std::time::Instant::now();
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1700.0, 1400.0))),
                ..Default::default()
            },
            |ctx| app.reading_window(ctx),
        );
        reading_ms.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    reading_ms.sort_by(f64::total_cmp);
    let p95_reading = reading_ms[(reading_ms.len() * 95 / 100).min(reading_ms.len() - 1)];
    let max_reading = *reading_ms.last().unwrap();

    println!(
        "WP10_PROFILE frame_p95_ms={p95_frame:.3} frame_max_ms={max_frame:.3} reading_p95_ms={p95_reading:.3} reading_max_ms={max_reading:.3}"
    );
    assert!(p95_frame <= 33.0, "网络帧 P95 {p95_frame:.3}ms 超过 33ms");
    assert!(
        p95_reading <= 200.0,
        "暖态资料切换 P95 {p95_reading:.3}ms 超过 200ms"
    );
}

#[test]
fn rename_preview_and_apply_are_two_explicit_ui_steps_with_undo() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    app.plan_target_rename(TargetRef::new("entity", "a"));
    app.rename_form.as_mut().unwrap().new_id = "alpha".into();

    click(&ctx, &mut app, 5, "预览重命名");
    assert!(app.rename_form.as_ref().unwrap().plan.is_some());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());

    click(&ctx, &mut app, 5, "应用跨视图重命名");
    assert!(app.rename_form.is_none(), "{:?}", app.io_error);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities
        .contains_key("alpha"));
    assert!(!app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities
        .contains_key("a"));
    assert_eq!(app.history.len(), 1);
    app.undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn template_fields_preserve_body_and_custom_values_and_suggestions_only_open_drafts() {
    let (ctx, mut app) = app();
    app.edit_entity(None);
    {
        let form = app.entity_editor.as_mut().unwrap();
        form.draft.display = "模板地点".into();
        form.draft.description = "第一段\n第二段正文".into();
        form.draft.properties.push((
            "custom_unknown".into(),
            worldline_core::ast::PropertyValue::Str("保留".into()),
        ));
    }
    click(&ctx, &mut app, 0, "创作模板 · 地理与地点");
    click(&ctx, &mut app, 0, "＋ 视觉与感官印象");
    {
        let form = app.entity_editor.as_mut().unwrap();
        let field = form
            .draft
            .properties
            .iter_mut()
            .find(|(key, _)| key == "place_1")
            .unwrap();
        field.1 = worldline_core::ast::PropertyValue::Str("海风与白石".into());
        form.draft.entity_type = "organization".into();
    }
    click(&ctx, &mut app, 0, "应用资料");
    let id = app.catalog_target.as_ref().unwrap().id.clone();
    let entity = &app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities[&id];
    assert_eq!(entity.description, "第一段\n第二段正文");
    assert_eq!(
        entity.properties["custom_unknown"],
        worldline_core::ast::PropertyValue::Str("保留".into())
    );
    assert_eq!(
        entity.properties["place_1"],
        worldline_core::ast::PropertyValue::Str("海风与白石".into())
    );

    let baseline = app.project.content_baseline();
    app.edit_entity(Some(&id));
    click(&ctx, &mut app, 0, "创作模板 · 组织与制度");
    click(&ctx, &mut app, 0, "任职于");
    assert!(app.relation_editor.is_some());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app
        .relation_editor
        .as_ref()
        .is_some_and(|form| form.draft.from == TargetRef::new("entity", &id)));
}

#[test]
fn collaboration_review_saves_comments_and_keeps_conflicting_proposals_open() {
    let (ctx, mut app) = app();
    app.new_comment_for_anchor(worldline_core::collaboration::CommentAnchor::Object {
        target: TargetRef::new("entity", "a"),
    });
    {
        let editor = app.review.comment_editor.as_mut().unwrap();
        editor.draft.author = "甲".into();
        editor.draft.body = "请补充来源".into();
    }
    click(&ctx, &mut app, 7, "保存批注");
    assert!(app.review.comment_editor.is_none(), "{:?}", app.io_error);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .comment_index
        .comments
        .contains_key("comment_1"));

    let entry = app.project.entry.clone();
    let mut proposed = app.project.document(&entry).unwrap().to_string();
    proposed.push_str("# 提案版本\n");
    app.project.set_text(&entry, proposed).unwrap();
    app.recompile();
    app.review.author = "乙".into();
    app.review.reason = "审阅正文修改".into();
    app.review.proposal_id = "proposal_ui".into();
    click(&ctx, &mut app, 7, "保存修改提案");
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .proposal_index
        .proposals
        .contains_key("proposal_ui"));

    let mut current = app.project.document(&entry).unwrap().to_string();
    current.push_str("# 并行当前修改\n");
    app.project.set_text(&entry, current).unwrap();
    app.recompile();
    let before = app.project.content_baseline();
    let output = frame(&ctx, &mut app, Vec::new(), 7);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("当前差异已过期"), "{rendered}");
    click(&ctx, &mut app, 7, "重新比较提案");
    let output = frame(&ctx, &mut app, Vec::new(), 7);
    let mut compared = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut compared);
    }
    assert!(compared.contains("变化："), "{compared}");
    assert!(compared.contains("基底"), "{compared}");
    assert!(compared.contains("当前"), "{compared}");
    assert!(compared.contains("提议"), "{compared}");
    click(&ctx, &mut app, 7, "采纳提案");
    assert_eq!(app.project.content_baseline(), before);
    assert_eq!(
        app.snapshot.as_ref().unwrap().proposal_index.proposals["proposal_ui"]
            .draft
            .status,
        worldline_core::collaboration::ProposalStatus::Open
    );
}

#[test]
fn narrow_review_switches_between_selectable_three_way_text() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    app.recompile();
    let entry = app.project.entry.clone();
    let original = app.project.document(&entry).unwrap().to_string();
    let mut proposed = original.clone();
    proposed.push_str("entity c kind place as \"新增地点\"\n");
    app.project.set_text(&entry, proposed).unwrap();
    app.recompile();
    app.review.author = "乙".into();
    app.review.reason = "检查窄屏差异".into();
    click(&ctx, &mut app, 7, "保存修改提案");
    app.project.set_text(&entry, original.clone()).unwrap();
    app.recompile();
    click(&ctx, &mut app, 14, "重新比较提案");
    for _ in 0..4 {
        let _ = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos2(200.0, 450.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: vec2(0.0, -8.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            14,
        );
    }
    let output = frame(&ctx, &mut app, Vec::new(), 14);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("基底"), "{rendered}");
    assert!(rendered.contains("当前"), "{rendered}");
    assert!(rendered.contains("提议"), "{rendered}");
    click(&ctx, &mut app, 14, "提议");
    let output = frame(&ctx, &mut app, Vec::new(), 14);
    let mut proposed_view = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut proposed_view);
    }
    assert!(proposed_view.contains("新增地点"), "{proposed_view}");
    let history_before_apply = app.history.len();
    click(&ctx, &mut app, 14, "采纳提案");
    assert!(
        app.project.document(&entry).unwrap().contains("新增地点"),
        "{:?}",
        app.io_error
    );
    assert_eq!(app.history.len(), history_before_apply + 1);
    app.undo(false);
    assert_eq!(app.project.document(&entry).unwrap(), original);
}
