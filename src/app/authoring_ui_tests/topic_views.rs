use super::*;

const TOPIC_FIXTURE: &str = r#"
character lin as "林舟"
character mei as "梅"
entity mei kind place as "梅"
entity guild_one kind organization as "灯会"
entity guild_two kind organization as "学会"
entity harbor kind place as "雾港"
period era as "旧纪元"
period past as "旧时代"
relation_type biological_parent as "亲生关系"
relation_type adoptive_parent as "收养关系"
relation_type member_of as "组织成员"
relation_type happens_at as "发生地点"
relation_def birth_edge type biological_parent from character lin to character mei
relation_def adoption_edge type adoptive_parent from character lin to character mei
relation_def parent_cycle type biological_parent from character mei to character lin
relation_def group_one type member_of from character lin to entity guild_one
  scope period era
relation_def group_two type member_of from character lin to entity guild_two
  scope period past
event alpha with lin during era
  有明确时段的第一条记录。
  -> END
event beta with lin during era
  有明确时段的并列记录。
  -> END
event later with lin during era follows alpha
  有明确先后约束的记录。
  -> END
event undated with lin
  文献没有日期。
  -> END
event mention with lin
  [[entity:harbor|正文提到雾港]]
  -> END
relation_def alpha_at type happens_at from event alpha to entity harbor
relation_def undated_at type happens_at from event undated to entity harbor
relation_def reverse_at type happens_at from entity harbor to event beta
"#;

fn install_source(app: &mut WorldeditApp, source: &str) {
    let entry = app.project.entry.clone();
    app.project.set_text(&entry, source.to_owned()).unwrap();
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:#?}",
        app.snapshot.as_ref().unwrap().result.diagnostics
    );
}

fn install_topic_fixture(app: &mut WorldeditApp) {
    install_source(app, TOPIC_FIXTURE);
}

#[test]
fn family_and_organization_views_map_filter_and_navigate_without_writes() {
    let (ctx, mut editor) = app();
    install_topic_fixture(&mut editor);
    let baseline = editor.project.content_baseline();
    let history_len = editor.history.len();
    let entry = editor.project.entry.clone();
    editor.open_network(TargetRef::new("character", "lin"));
    let _ = frame(&ctx, &mut editor, Vec::new(), 4);
    let positions = editor.network_state.positions.clone();

    click(&ctx, &mut editor, 4, "家族关系");
    let unmapped = rendered_text_in_window(&ctx, &mut editor, 4, "尚未为任何关系类型填写角色标签");
    assert!(!unmapped.contains("阅读关系 birth_edge"));
    replace_text_area(&ctx, &mut editor, 4, "角色标签 · biological_parent", "生亲");
    replace_text_area(&ctx, &mut editor, 4, "角色标签 · adoptive_parent", "养亲");
    let family = rendered_text_in_window(&ctx, &mut editor, 4, "明确家族与亲属关系");
    assert!(family.contains("生亲"));
    assert!(family.contains("养亲"));
    assert!(family.contains("当前页存在关系环"));
    assert!(family.contains("梅"));
    click(&ctx, &mut editor, 4, "人物 · 梅 · mei");
    assert_eq!(
        editor.reading_target,
        Some(TargetRef::new("character", "mei"))
    );

    let relation = editor
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .relations["birth_edge"]
        .clone();
    click(&ctx, &mut editor, 4, "阅读关系 birth_edge");
    assert_eq!(
        editor.reading_target,
        Some(TargetRef::new("relation", "birth_edge"))
    );
    click(
        &ctx,
        &mut editor,
        4,
        &format!("定位来源 {}:{}", relation.file, relation.line),
    );
    assert_eq!(editor.active_file, entry);
    assert_eq!(editor.tab, Tab::Edit);

    click(&ctx, &mut editor, 4, "组织归属");
    replace_text_area(&ctx, &mut editor, 4, "角色标签 · member_of", "组织成员");
    let organizations = rendered_text_in_window(&ctx, &mut editor, 4, "学会");
    assert!(organizations.contains("灯会"));
    assert!(organizations.contains("学会"));
    replace_text_area(
        &ctx,
        &mut editor,
        4,
        "kind:id，例如 period:era",
        "period:era",
    );
    click(&ctx, &mut editor, 4, "加入范围");
    let scoped = rendered_text_in_window(&ctx, &mut editor, 4, "灯会");
    assert!(scoped.contains("灯会"));
    assert!(!scoped.contains("学会"));

    let mut narrow_text = String::new();
    for _ in 0..20 {
        let narrow = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(760.0, 720.0),
                )),
                events: vec![
                    egui::Event::PointerMoved(egui::pos2(300.0, 360.0)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -400.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| editor.network_tab(ctx),
        );
        narrow_text.clear();
        for shape in &narrow.shapes {
            collect_text(&shape.shape, &mut narrow_text);
        }
        if narrow_text.contains("窄屏使用下方可访问关系列表") {
            break;
        }
    }
    assert!(
        narrow_text.contains("窄屏使用下方可访问关系列表"),
        "{narrow_text}"
    );
    assert_eq!(editor.project.content_baseline(), baseline);
    assert_eq!(editor.history.len(), history_len);
    assert_eq!(editor.network_state.positions, positions);
}

#[test]
fn character_and_place_history_show_unknown_parallel_and_explicit_sources() {
    let (ctx, mut editor) = app();
    install_topic_fixture(&mut editor);
    editor.open_network(TargetRef::new("character", "lin"));
    click(&ctx, &mut editor, 4, "人物 / 地点历史");
    let mut character_history = String::new();
    for label in [
        "时间未知 · core 未补日期或时段",
        "同层并列 · 不代表同时发生",
        "明确先后约束",
        "成员来源：正文显式 with",
    ] {
        character_history.push_str(&scroll_at_to_visible(
            &ctx,
            &mut editor,
            4,
            pos2(850.0, 800.0),
            label,
        ));
        assert!(character_history.contains(label), "{character_history}");
    }
    let undated = editor
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .object(&TargetRef::new("event", "undated"))
        .unwrap()
        .clone();
    scroll_at_to_visible(
        &ctx,
        &mut editor,
        4,
        pos2(850.0, 800.0),
        &format!("阅读 事件 · {} · undated", undated.display),
    );
    click(
        &ctx,
        &mut editor,
        4,
        &format!("阅读 事件 · {} · undated", undated.display),
    );
    assert_eq!(
        editor.reading_target,
        Some(TargetRef::new("event", "undated"))
    );
    scroll_at_to_visible(
        &ctx,
        &mut editor,
        4,
        pos2(850.0, 800.0),
        &format!("定位事件 {}:{}", undated.file, undated.line),
    );
    click(
        &ctx,
        &mut editor,
        4,
        &format!("定位事件 {}:{}", undated.file, undated.line),
    );
    assert_eq!(editor.active_file, editor.project.entry);

    editor.open_network(TargetRef::new("entity", "harbor"));
    click(&ctx, &mut editor, 4, "人物 / 地点历史");
    scroll_at_to_visible(
        &ctx,
        &mut editor,
        4,
        pos2(850.0, 800.0),
        "角色标签 · happens_at",
    );
    replace_text_area(&ctx, &mut editor, 4, "角色标签 · happens_at", "发生地点");
    let place_history = tall_network_frame(&ctx, &mut editor, Vec::new());
    let mut place_text = String::new();
    for shape in &place_history.shapes {
        collect_text(&shape.shape, &mut place_text);
    }
    assert!(place_text.contains("alpha_at"));
    assert!(place_text.contains("undated_at"));
    assert!(place_text.contains("时间未知 · core 未补日期或时段"));
    assert!(!place_text.contains("正文提到雾港"));
    assert!(place_text.contains("发生地点 · 类型 happens_at"));
}

#[test]
fn topic_mapping_does_not_follow_the_egui_context_into_another_project() {
    let (ctx, mut first) = app();
    install_topic_fixture(&mut first);
    first.open_network(TargetRef::new("character", "lin"));
    click(&ctx, &mut first, 4, "家族关系");
    replace_text_area(&ctx, &mut first, 4, "角色标签 · biological_parent", "生亲");
    assert!(first.topic_view_active(&ctx));
    let first_baseline = first.project.content_baseline();
    first.reset_views();
    first.open_network(TargetRef::new("character", "lin"));
    let _ = frame(&ctx, &mut first, Vec::new(), 4);
    assert!(!first.topic_view_active(&ctx));
    assert_eq!(first.project.content_baseline(), first_baseline);

    let (_, mut second) = app();
    install_topic_fixture(&mut second);
    second.open_network(TargetRef::new("character", "lin"));
    let baseline = second.project.content_baseline();
    let output = frame(&ctx, &mut second, Vec::new(), 4);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(!second.topic_view_active(&ctx));
    assert!(rendered.contains("局部关系网络"));
    assert_eq!(second.project.content_baseline(), baseline);
}
fn tall_network_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1700.0, 100_000.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| app.network_tab(ctx),
    )
}

fn click_tall_network_button(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let output = tall_network_frame(ctx, app, Vec::new());
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position(&shape.shape, label))
        .unwrap_or_else(|| panic!("按钮未显示：{label}"));
    for pressed in [true, false] {
        let _ = tall_network_frame(
            ctx,
            app,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

#[test]
fn topic_relation_pages_continue_with_the_explicit_role_mapping() {
    let (ctx, mut editor) = app();
    let mut source =
        String::from("character root\ncharacter leaf\nrelation_type parent as \"亲属\"\n");
    for index in 0..501 {
        source.push_str(&format!(
            "relation_def edge_{index:03} type parent from character root to character leaf\n"
        ));
    }
    install_source(&mut editor, &source);
    let baseline = editor.project.content_baseline();
    editor.open_network(TargetRef::new("character", "root"));
    click(&ctx, &mut editor, 4, "家族关系");
    replace_text_area(&ctx, &mut editor, 4, "角色标签 · parent", "家人");
    let first_page = rendered_text_in_window(&ctx, &mut editor, 4, "关系页已截断");
    assert!(first_page.contains("阅读关系 edge_000"));
    click_tall_network_button(&ctx, &mut editor, "继续读取关系");
    let next_page = rendered_text_in_window(&ctx, &mut editor, 4, "阅读关系 edge_500");
    assert!(next_page.contains("家人"));
    assert!(!next_page.contains("阅读关系 edge_000"));
    assert_eq!(editor.project.content_baseline(), baseline);
}
