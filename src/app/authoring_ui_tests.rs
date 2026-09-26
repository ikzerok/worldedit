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
    let root = app.project.root.clone();
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

fn register_project_template(app: &mut WorldeditApp, document: &str) {
    let parsed: serde_json::Value = serde_json::from_str(document).unwrap();
    let id = parsed["id"].as_str().unwrap();
    let slug = id.strip_prefix("project:").unwrap();
    let relative = format!(".world/templates/{slug}.json");
    let manifest_path = app.project.root.join(".world/project.json");
    let mut manifest: serde_json::Value = serde_json::from_slice(
        app.project
            .authoring_document(&manifest_path)
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["required_features"] = serde_json::json!([
        "content.entities.v1",
        "content.relations.v1",
        "content.templates.v1",
        "content.object_refs.v1"
    ]);
    manifest["templates"][id] = relative.clone().into();
    app.project
        .set_authoring_document(&manifest_path, serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    app.project
        .create_authoring_document(
            &app.project.root.join(relative),
            document.as_bytes().to_vec(),
        )
        .unwrap();
    app.recompile();
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

fn reader_publish_app() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app();
    app.project
        .set_text(
            &app.active_file.clone(),
            concat!(
                "event public as \"Public Event\"\n",
                "  Published story body.\n",
                "  [[event:private|HIDDEN_LINK_LABEL_SENTINEL]]\n",
                "  -> END\n",
                "event private as \"HIDDEN_EVENT_TITLE_SENTINEL\"\n",
                "  HIDDEN_EVENT_BODY_SENTINEL.\n",
                "  -> END\n",
                "asset cover image \"assets/cover.png\" as \"Public Cover\"\n",
                "asset hidden file \"private/secret.txt\" as \"HIDDEN_ASSET_TITLE_SENTINEL\"\n",
            )
            .into(),
        )
        .unwrap();
    app.project
        .set_authoring_document(
            &app.project.root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.10","required_features":["content.entities.v1","content.relations.v1","presentation.manuscripts.v1"],"maps":{},"graph_views":{},"manuscripts":{"public-book":".world/manuscripts/public-book.json"}}"#.to_vec(),
        )
        .unwrap();
    app.project
        .create_authoring_document(
            &app.project.root.join(".world/manuscripts/public-book.json"),
            br#"{"schema_version":1,"id":"public-book","title":"Public Book","entries":[{"id":"public-chapter","kind":"chapter","title":"Public Chapter","target_ref":{"kind":"event","id":"public"},"summary":"Public chapter summary","status":"published"},{"id":"hidden-chapter","kind":"chapter","title":"HIDDEN_CHAPTER_TITLE_SENTINEL","target_ref":{"kind":"event","id":"private"},"summary":"HIDDEN_CHAPTER_BODY_SENTINEL","status":"draft"}]}"#.to_vec(),
        )
        .unwrap();
    app.project.save().unwrap();
    std::fs::create_dir_all(app.project.root.join("assets")).unwrap();
    std::fs::create_dir_all(app.project.root.join("private")).unwrap();
    std::fs::write(app.project.root.join("assets/cover.png"), [0, 1, 255, 2]).unwrap();
    std::fs::write(
        app.project.root.join("private/secret.txt"),
        b"HIDDEN_ATTACHMENT_BYTES_SENTINEL",
    )
    .unwrap();
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.snapshot.as_ref().unwrap().result.diagnostics
    );
    assert!(
        app.project
            .manuscript_indices()
            .values()
            .all(|index| index.diagnostics.is_empty()),
        "{:?}",
        app.project
            .manuscript_indices()
            .values()
            .flat_map(|index| index.diagnostics.iter())
            .collect::<Vec<_>>()
    );
    (ctx, app)
}

fn wait_for_reader_publish(ctx: &egui::Context, app: &mut WorldeditApp) {
    let mut last = String::new();
    for _ in 0..500 {
        let output = frame(ctx, app, Vec::new(), 26);
        last.clear();
        for shape in &output.shapes {
            collect_text(&shape.shape, &mut last);
        }
        if last.contains("发布 ZIP") || last.contains("预览失败：") {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("reader publish preview did not finish: {last}");
}

fn assert_reader_package_resources_resolve(files: &crate::archive::Files) {
    for (page, bytes) in files {
        if page.extension().is_none_or(|extension| extension != "html") {
            continue;
        }
        let html = std::str::from_utf8(bytes).unwrap();
        assert!(
            html.starts_with("<!doctype html>"),
            "invalid reader page {page:?}"
        );
        for attribute in ["href=\"", "src=\""] {
            let mut rest = html;
            while let Some(start) = rest.find(attribute) {
                rest = &rest[start + attribute.len()..];
                let end = rest.find('"').expect("closed resource URL");
                let url = &rest[..end];
                assert!(!url.contains("://"), "reader package has remote URL {url}");
                assert!(
                    !url.starts_with('/'),
                    "reader URL must be package-relative: {url}"
                );
                let mut resolved = page
                    .parent()
                    .unwrap_or(std::path::Path::new(""))
                    .to_path_buf();
                for component in std::path::Path::new(url).components() {
                    match component {
                        std::path::Component::Normal(value) => resolved.push(value),
                        std::path::Component::ParentDir => assert!(
                            resolved.pop(),
                            "reader URL escapes package root: {page:?} -> {url}"
                        ),
                        std::path::Component::CurDir => {}
                        _ => panic!("unsafe reader URL {page:?} -> {url}"),
                    }
                }
                assert!(
                    files.contains_key(&resolved),
                    "missing reader resource {page:?} -> {url}"
                );
                rest = &rest[end + 1..];
            }
        }
    }
}
fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
    mut window: u8,
) -> egui::FullOutput {
    if window == 17 && app.markdown_import_wizard.is_some() {
        window = 25;
    }
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                if window == 16 || window == 21 || window == 24 {
                    vec2(700.0, 640.0)
                } else if window == 25 || window == 26 {
                    vec2(1280.0, 1000.0)
                } else if window == 9 {
                    vec2(1040.0, 660.0)
                } else if window == 19 {
                    vec2(2600.0, 2400.0)
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
            16 => app.review_tab(ctx),
            8 | 9 => app.reading_window(ctx),
            11 => app.source_tab(ctx),
            13 => app.manuscript_tab(ctx),
            17 | 19 => app.template_manager_tab(ctx),
            18 => app.sidebar(ctx),
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
            20 | 21 => app.play_tab(ctx),
            22 => app.canvas_tab(ctx),
            23 => app.checkpoint_history_tab(ctx),
            24 => app.checkpoint_history_tab(ctx),
            25 => {
                app.top_bar(ctx);
                app.markdown_import_window(ctx);
            }
            26 => {
                app.top_bar(ctx);
                app.reader_publish_window(ctx);
            }
            _ => app.content_deletion_window(ctx),
        },
    )
}
fn replay_app(source: &str, window: u8) -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app();
    app.project
        .set_text(&app.active_file.clone(), source.into())
        .unwrap();
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.snapshot.as_ref().unwrap().result.diagnostics
    );
    app.tab = super::Tab::Play;
    click(&ctx, &mut app, window, "▶ 开始试玩");
    (ctx, app)
}
fn wait_for_replay(ctx: &egui::Context, app: &mut WorldeditApp) {
    for _ in 0..500 {
        let _ = frame(ctx, app, Vec::new(), 20);
        if app.replay_debugger.job.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn play_tab_exposes_replay_capture_and_condition_inspection_controls() {
    let (ctx, mut app) = app();
    let source =
        "event start\n  choice \"继续\" if false\n    -> END\n  choice \"结束\"\n    -> END\n";
    app.project
        .set_text(&app.active_file.clone(), source.into())
        .unwrap();
    app.recompile();
    app.tab = super::Tab::Play;
    click(&ctx, &mut app, 20, "▶ 开始试玩");

    let output = frame(&ctx, &mut app, Vec::new(), 20);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("保存当前路径"), "{rendered}");
    assert!(rendered.contains("解释当前条件"), "{rendered}");
    assert!(rendered.contains("种子"), "{rendered}");
}

#[test]
fn named_recorded_path_replays_against_the_current_snapshot_without_project_writes() {
    let source = concat!(
        "let score = 0\n",
        "event start\n",
        "  choice \"继续\"\n",
        "    set score = score + 1\n",
        "    -> END\n",
    );
    let (ctx, mut app) = replay_app(source, 20);
    let baseline = app.project.content_baseline();
    let history_len = app.history.len();

    click(&ctx, &mut app, 20, "选择：继续");
    click(&ctx, &mut app, 20, "● 保存当前路径");
    assert_eq!(app.replay_debugger.saved_paths.len(), 1);
    assert_eq!(app.replay_debugger.saved_paths[0].trace.steps.len(), 1);
    assert!(app.replay_debugger.saved_paths[0].trace.complete);

    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    wait_for_replay(&ctx, &mut app);
    let result = app.replay_debugger.result.as_ref().unwrap();
    assert!(matches!(
        result.status,
        worldline_runtime::ReplayStatus::Replayed {
            ended: true,
            complete: true
        }
    ));
    assert!(result.state_diff.contains_key("vars"));
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.history.len(), history_len);
}

#[test]
fn condition_explanation_uses_the_real_ui_action_without_advancing_story() {
    let source = concat!(
        "event start\n",
        "  choice \"不可选\" if false\n",
        "    -> END\n",
        "  choice \"继续\"\n",
        "    -> END\n",
    );
    let (ctx, mut app) = replay_app(source, 20);
    let _ = frame(&ctx, &mut app, Vec::new(), 20);
    let story = app.play.as_ref().unwrap().story.as_ref().unwrap();
    let before_trace = story.replay_trace();
    let before_state = story.state_view();
    let before_turns = story.turns();
    let baseline = app.project.content_baseline();

    click(&ctx, &mut app, 20, "解释当前条件（只读）");

    let story = app.play.as_ref().unwrap().story.as_ref().unwrap();
    assert_eq!(story.replay_trace(), before_trace);
    assert_eq!(story.state_view(), before_state);
    assert_eq!(story.turns(), before_turns);
    let explanations = app.replay_debugger.explanations.as_ref().unwrap();
    assert!(explanations.iter().any(|item| {
        item.choice.label == "不可选" && !item.available && item.unavailable_reason.is_some()
    }));
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn stale_recorded_choice_stops_and_ui_jumps_to_its_source_line() {
    let source = "event start\n  choice \"旧选择\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    click(&ctx, &mut app, 20, "选择：旧选择");
    click(&ctx, &mut app, 20, "● 保存当前路径");

    app.project
        .set_text(
            &app.active_file.clone(),
            "event start\n  choice \"新选择\"\n    -> END\n".into(),
        )
        .unwrap();
    app.recompile();
    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    wait_for_replay(&ctx, &mut app);

    let result = app.replay_debugger.result.as_ref().unwrap();
    let worldline_runtime::ReplayStatus::Diverged {
        reason,
        expected_choice,
        actual_choices,
        ..
    } = &result.status
    else {
        panic!("应停止在缺失的旧选择，实际结果：{:?}", result.status);
    };
    assert!(reason.contains("不匹配") || reason.contains("不存在"));
    assert!(expected_choice.is_none());
    assert_eq!(
        app.replay_debugger.saved_paths[0].trace.steps[0]
            .choice
            .label,
        "旧选择"
    );
    assert_eq!(actual_choices[0].label, "新选择");
    let actual_line = actual_choices[0].line;
    click(&ctx, &mut app, 20, "跳转到失败位置");
    assert_eq!(app.tab, super::Tab::Edit);
    assert_eq!(app.jump, Some((actual_line, 1)));
}

#[test]
fn removed_choice_stops_without_fallback_and_locates_the_recorded_source_line() {
    let source = "event start\n  choice \"待删除\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    click(&ctx, &mut app, 20, "选择：待删除");
    click(&ctx, &mut app, 20, "● 保存当前路径");

    app.project
        .set_text(
            &app.active_file.clone(),
            "event start\n  选择已经删除。\n  -> END\n".into(),
        )
        .unwrap();
    app.recompile();
    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    wait_for_replay(&ctx, &mut app);
    let result = app.replay_debugger.result.as_ref().unwrap();
    let worldline_runtime::ReplayStatus::Diverged { actual_choices, .. } = &result.status else {
        panic!("删除的选择必须停止并报告分歧：{:?}", result.status);
    };
    assert!(actual_choices.is_empty());

    let line = app.replay_debugger.saved_paths[0].trace.steps[0]
        .choice
        .line;
    click(&ctx, &mut app, 20, "跳转到失败位置");
    assert_eq!(app.tab, super::Tab::Edit);
    assert_eq!(app.jump, Some((line, 1)));
}

#[test]
fn changed_node_stops_at_the_new_node_instead_of_reusing_the_old_choice_index() {
    let source = "event start\n  choice \"继续\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    click(&ctx, &mut app, 20, "选择：继续");
    click(&ctx, &mut app, 20, "● 保存当前路径");

    app.project
        .set_text(
            &app.active_file.clone(),
            "event replacement\n  choice \"继续\"\n    -> END\n".into(),
        )
        .unwrap();
    app.recompile();
    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    wait_for_replay(&ctx, &mut app);

    let result = app.replay_debugger.result.as_ref().unwrap();
    let worldline_runtime::ReplayStatus::Diverged { actual_choices, .. } = &result.status else {
        panic!("修改节点后应报告路径分歧：{:?}", result.status);
    };
    assert_eq!(actual_choices[0].node, "replacement");
    assert_eq!(
        app.replay_debugger.saved_paths[0].trace.steps[0]
            .choice
            .node,
        "start"
    );
}

#[test]
fn narrow_play_view_can_switch_between_body_and_debug_information() {
    let source = "event start\n  choice \"继续\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 21);
    click(&ctx, &mut app, 21, "调试信息");
    let output = frame(&ctx, &mut app, Vec::new(), 21);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("叙事调试信息"), "{rendered}");

    click(&ctx, &mut app, 21, "正文");
    let output = frame(&ctx, &mut app, Vec::new(), 21);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("正文"), "{rendered}");
}

#[test]
fn event_graph_marks_only_visits_from_the_selected_replay() {
    let source = "event start\n  choice \"继续\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    click(&ctx, &mut app, 20, "选择：继续");
    click(&ctx, &mut app, 20, "● 保存当前路径");
    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    wait_for_replay(&ctx, &mut app);

    app.tab = super::Tab::Graph;
    let output = frame(&ctx, &mut app, Vec::new(), 22);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("访问 ×1"), "{rendered}");
    assert!(rendered.contains("无标记表示未测试"), "{rendered}");
}

#[test]
fn imported_trace_size_limit_is_enforced_without_rendering_or_retaining_it() {
    let source = "event start\n  choice \"继续\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    app.replay_debugger.import_json = "x".repeat(1024 * 1024 + 1);
    click(&ctx, &mut app, 20, "拒绝超限轨迹");

    assert!(app.replay_debugger.saved_paths.is_empty());
    assert!(app
        .replay_debugger
        .notice
        .as_deref()
        .is_some_and(|notice| notice.contains("1 MiB")));
}

#[test]
fn replay_cancellation_button_cancels_a_long_recorded_trace_and_keeps_it() {
    let source = "event start\n  choice \"再来\"\n    -> start\n";
    let (ctx, mut app) = replay_app(source, 20);
    let snapshot = app.snapshot.as_ref().unwrap();
    let mut story = worldline_runtime::Story::new_with_seed(
        &snapshot.result.program,
        &snapshot.result.analysis,
        7,
    )
    .unwrap();
    story.continue_story().unwrap();
    for _ in 0..20_000 {
        story.choose(0).unwrap();
        story.continue_story().unwrap();
    }
    let trace = story.replay_trace();
    assert_eq!(trace.steps.len(), 20_000);
    app.replay_debugger
        .saved_paths
        .push(super::SavedReplayPath {
            name: "长路径".into(),
            trace,
        });
    app.replay_debugger.selected_path = Some(0);
    app.replay_debugger.max_steps = 1_000_000_000;
    app.replay_debugger.time_budget_ms = 600_000;

    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    click_without_settling(&ctx, &mut app, 20, "取消重放");
    wait_for_replay(&ctx, &mut app);

    assert!(matches!(
        app.replay_debugger.result.as_ref().unwrap().status,
        worldline_runtime::ReplayStatus::Cancelled
    ));
    assert_eq!(app.replay_debugger.saved_paths[0].trace.steps.len(), 20_000);
    assert!(app.history.is_empty());
}

#[test]
fn pause_stop_and_checkpoint_import_controls_keep_debug_state_out_of_project() {
    let source = "event start\n  choice \"继续\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    let _ = frame(&ctx, &mut app, Vec::new(), 20);
    let baseline = app.project.content_baseline();
    let story = app.play.as_mut().unwrap().story.as_mut().unwrap();
    story.start_trace_from_here().unwrap();
    let checkpoint_trace = story.replay_trace();
    assert!(matches!(
        checkpoint_trace.origin,
        worldline_runtime::ReplayOrigin::Checkpoint { .. }
    ));
    app.replay_debugger.import_json = serde_json::to_string(&checkpoint_trace).unwrap();
    click(&ctx, &mut app, 20, "检查并导入路径");
    assert_eq!(app.replay_debugger.saved_paths.len(), 1);
    assert!(matches!(
        app.replay_debugger.saved_paths[0].trace.origin,
        worldline_runtime::ReplayOrigin::Checkpoint { .. }
    ));
    click(&ctx, &mut app, 20, "查看检查点状态");
    let output = frame(&ctx, &mut app, Vec::new(), 20);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("检查点起点"), "{rendered}");

    click(&ctx, &mut app, 20, "Ⅱ 暂停");
    assert!(app.play.as_ref().unwrap().paused);
    click(&ctx, &mut app, 20, "▶ 继续");
    assert!(!app.play.as_ref().unwrap().paused);
    click(&ctx, &mut app, 20, "■ 停止");
    assert!(app.play.as_ref().unwrap().ended);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

fn markdown_import_fixture(markdown: &[u8]) -> (std::path::PathBuf, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "worldedit-markdown-import-ui-{}-{}",
        std::process::id(),
        NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    let source = root.join("source");
    let target = root.join("empty-target");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(source.join("page.md"), markdown).unwrap();
    (source, target)
}

fn open_markdown_import(ctx: &egui::Context, app: &mut WorldeditApp) {
    click(ctx, app, 25, "工程");
    click(ctx, app, 25, "导入 Markdown…");
}

#[test]
fn markdown_import_preview_and_cancel_keep_the_target_empty_and_show_losses() {
    let (ctx, mut app) = app();
    let (source, target) = markdown_import_fixture(
        b"---\ntitle: Imported Page\ncustom_field: keep\n---\n\nA **formatted** paragraph.\n",
    );
    let baseline = app.project.content_baseline();
    open_markdown_import(&ctx, &mut app);
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择 Markdown 来源目录…",
        &source.display().to_string(),
    );
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择空工程目录…",
        &target.display().to_string(),
    );
    click(&ctx, &mut app, 17, "预检导入");
    scroll_rendered_text(&ctx, &mut app, 17, "损失预览 ·");
    click_containing(&ctx, &mut app, 17, "损失预览 ·");
    let preview = format!(
        "{}{}",
        scroll_rendered_text(&ctx, &mut app, 17, "UNSUPPORTED_FRONT_MATTER_FIELD"),
        scroll_rendered_text(&ctx, &mut app, 17, "UNSUPPORTED_INLINE_MARKUP")
    );

    assert!(
        preview.contains("UNSUPPORTED_FRONT_MATTER_FIELD"),
        "{preview}"
    );
    assert!(preview.contains("UNSUPPORTED_INLINE_MARKUP"), "{preview}");
    assert!(preview.contains("page.md"), "{preview}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(!target.join(".world").exists());
    assert_eq!(
        std::fs::read(source.join("page.md")).unwrap(),
        b"---\ntitle: Imported Page\ncustom_field: keep\n---\n\nA **formatted** paragraph.\n"
    );

    click(&ctx, &mut app, 17, "取消");
    assert!(!target.join(".world").exists());
    assert!(std::fs::read_dir(&target).unwrap().next().is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    std::fs::remove_dir_all(source.parent().unwrap()).unwrap();
}

#[test]
fn markdown_import_files_snapshot_preflight_uses_core_and_cancel_keeps_targets_unchanged() {
    let (ctx, mut app) = app();
    let markdown =
        b"---\ntitle: Snapshot Page\ncustom_field: keep\n---\n\nA **formatted** paragraph.\n";
    let (source, target) = markdown_import_fixture(markdown);
    open_markdown_import(&ctx, &mut app);
    app.markdown_import_wizard
        .as_mut()
        .unwrap()
        .set_source_files(worldline_core::workspace_snapshot::Files::from([(
            std::path::PathBuf::from("page.md"),
            markdown.to_vec(),
        )]));
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择空工程目录…",
        &target.display().to_string(),
    );
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 17, "预检导入");

    let preview = rendered_text_in_window(&ctx, &mut app, 17, "预检完成：");
    assert!(preview.contains("1 个页面"), "{preview}");
    assert!(preview.contains("3 个损失"), "{preview}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(std::fs::read_dir(&target).unwrap().next().is_none());

    click(&ctx, &mut app, 17, "取消");
    assert!(std::fs::read_dir(&target).unwrap().next().is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    std::fs::remove_dir_all(source.parent().unwrap()).unwrap();
}

#[test]
fn markdown_import_apply_requires_loss_confirmation_and_round_trips_original_markdown() {
    let (ctx, mut app) = app();
    let original =
        b"---\ntitle: Imported Page\ncustom_field: keep\n---\n\nA **formatted** paragraph.\n";
    let (source, target) = markdown_import_fixture(original);
    open_markdown_import(&ctx, &mut app);
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择 Markdown 来源目录…",
        &source.display().to_string(),
    );
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择空工程目录…",
        &target.display().to_string(),
    );
    click(&ctx, &mut app, 17, "预检导入");
    scroll_rendered_text(&ctx, &mut app, 17, "损失预览 ·");
    click_containing(&ctx, &mut app, 17, "损失预览 ·");
    click(&ctx, &mut app, 17, "应用导入");
    assert!(std::fs::read_dir(&target).unwrap().next().is_none());
    assert!(
        rendered_text_in_window(&ctx, &mut app, 17, "我已检查并接受预览中的损失")
            .contains("我已检查并接受预览中的损失")
    );

    click(&ctx, &mut app, 17, "我已检查并接受预览中的损失");
    click(&ctx, &mut app, 17, "我已检查目标语言版本升级及其影响");
    click(&ctx, &mut app, 17, "应用导入");

    assert_eq!(app.project.root, Project::open(&target).unwrap().root);
    assert!(app.saved_location);
    assert!(app.history.is_empty());
    assert!(app
        .message
        .as_deref()
        .is_some_and(|message| message.contains("1 个页面")));
    let mut reopened = Project::open(&target).unwrap();
    assert!(!reopened.compile().has_errors());
    assert!(reopened
        .export_files()
        .unwrap()
        .values()
        .any(|bytes| bytes.as_slice() == original));
    std::fs::remove_dir_all(source.parent().unwrap()).unwrap();
}

#[test]
fn markdown_import_rejects_a_source_that_changes_after_preview_without_writing() {
    let (ctx, mut app) = app();
    let (source, target) = markdown_import_fixture(
        b"---\ntitle: Stable Page\n---\n\nPlain text without conversion losses.\n",
    );
    open_markdown_import(&ctx, &mut app);
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择 Markdown 来源目录…",
        &source.display().to_string(),
    );
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择空工程目录…",
        &target.display().to_string(),
    );
    click(&ctx, &mut app, 17, "预检导入");
    assert!(rendered_text_in_window(&ctx, &mut app, 17, "应用导入").contains("应用导入"));

    let changed_source =
        b"---\ntitle: Changed Page\n---\n\nPlain text without conversion losses.\n";
    std::fs::write(source.join("page.md"), changed_source).unwrap();
    click(&ctx, &mut app, 17, "我已检查目标语言版本升级及其影响");
    let before_apply = rendered_text_in_window(&ctx, &mut app, 17, "应用导入");
    assert!(!before_apply.contains("尚未确认语言升级"), "{before_apply}");
    if before_apply.contains("我已检查并接受预览中的损失") {
        click(&ctx, &mut app, 17, "我已检查并接受预览中的损失");
    }
    let before_stale_apply = rendered_text_in_window(&ctx, &mut app, 17, "应用导入");
    assert!(
        !before_stale_apply.contains("尚未确认损失"),
        "{before_stale_apply}"
    );
    click(&ctx, &mut app, 17, "应用导入");
    let rendered = rendered_text_in_window(&ctx, &mut app, 17, "已过期");

    assert!(rendered.contains("已过期"), "{rendered}");
    assert!(rendered.contains("重新预检"), "{rendered}");
    assert!(!target.join(".world").exists());
    assert!(std::fs::read_dir(&target).unwrap().next().is_none());
    click(&ctx, &mut app, 17, "重新预检");
    let ready = rendered_text_in_window(&ctx, &mut app, 17, "预检完成：");
    assert!(ready.contains("0 个阻塞冲突"), "{ready}");
    let ready_to_apply = rendered_text_in_window(&ctx, &mut app, 17, "必需确认已完成");
    assert!(
        ready_to_apply.contains("必需确认已完成"),
        "{ready_to_apply}"
    );
    click_containing(&ctx, &mut app, 17, "页面映射 ·");
    let refreshed = scroll_rendered_text(&ctx, &mut app, 17, "Changed Page");
    assert!(refreshed.contains("Changed Page"), "{refreshed}");
    scroll_window_to_top(&ctx, &mut app, 17);
    click(&ctx, &mut app, 17, "应用导入");
    assert_eq!(
        app.project.root,
        Project::open(&target).unwrap().root,
        "message={:?}, io_error={:?}",
        app.message,
        app.io_error
    );
    let mut reopened = Project::open(&target).unwrap();
    let result = reopened.compile();
    assert!(!result.has_errors());
    assert!(reopened
        .export_files()
        .unwrap()
        .values()
        .any(|bytes| bytes.as_slice() == changed_source));
    std::fs::remove_dir_all(source.parent().unwrap()).unwrap();
}

#[test]
fn markdown_import_resolves_a_real_id_conflict_without_merging_same_name_targets() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    app.saved_location = true;
    let root = app.project.root.clone();
    let (source, _) = markdown_import_fixture(
        "---\nid: a\ntitle: \"同名\"\n---\n\nA separately imported page.\n".as_bytes(),
    );
    open_markdown_import(&ctx, &mut app);
    click(&ctx, &mut app, 17, "导入到当前工程");
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        17,
        "选择 Markdown 来源目录…",
        &source.display().to_string(),
    );
    click(&ctx, &mut app, 17, "预检导入");
    scroll_rendered_text(&ctx, &mut app, 17, "阻塞冲突 ·");
    click_containing(&ctx, &mut app, 17, "阻塞冲突 ·");
    let conflict = format!(
        "{}{}",
        scroll_rendered_text(&ctx, &mut app, 17, "ENTITY_ID_CONFLICT"),
        scroll_rendered_text(&ctx, &mut app, 17, "a_import_2")
    );
    assert!(conflict.contains("page.md"), "{conflict}");
    assert!(conflict.contains("a_import_2"), "{conflict}");
    click_containing(&ctx, &mut app, 17, "阻塞冲突 ·");
    scroll_window_to_top(&ctx, &mut app, 17);
    scroll_rendered_text(&ctx, &mut app, 17, "同名资料提示 ·");
    click_containing(&ctx, &mut app, 17, "同名资料提示 ·");
    let name_conflict = scroll_rendered_text(&ctx, &mut app, 17, "工程中存在同名资料");
    assert!(name_conflict.contains("entity:a"), "{name_conflict}");
    assert!(name_conflict.contains("entity:b"), "{name_conflict}");

    scroll_window_to_top(&ctx, &mut app, 17);
    click_containing(&ctx, &mut app, 17, "同名资料提示 ·");
    click_containing(&ctx, &mut app, 17, "阻塞冲突 ·");
    scroll_rendered_text(&ctx, &mut app, 17, "a_import_2");
    click(&ctx, &mut app, 17, "a_import_2");
    click(&ctx, &mut app, 17, "预检导入");
    let ready = rendered_text_in_window(&ctx, &mut app, 17, "预检完成：");
    assert!(ready.contains("0 个阻塞冲突"), "{ready}");
    if ready.contains("我已检查并接受预览中的损失") {
        click(&ctx, &mut app, 17, "我已检查并接受预览中的损失");
    }
    if ready.contains("我已检查目标语言版本升级及其影响") {
        click(&ctx, &mut app, 17, "我已检查目标语言版本升级及其影响");
    }
    click_containing(&ctx, &mut app, 17, "页面映射 ·");
    let resolved = rendered_text_in_window(&ctx, &mut app, 17, "entity:a_import_2");
    assert!(resolved.contains("page.md"), "{resolved}");
    click(&ctx, &mut app, 17, "应用导入");

    assert!(!app.project.is_dirty());
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert!(app
        .message
        .as_deref()
        .is_some_and(|message| message.contains("导入已保存")));
    let mut reopened = Project::open(&root).unwrap();
    let result = reopened.compile();
    assert!(!result.has_errors());
    assert!(result.analysis.catalog.entities.contains_key("a_import_2"));
    assert!(result.analysis.catalog.entities.contains_key("a"));
    std::fs::remove_dir_all(source.parent().unwrap()).unwrap();
    std::fs::remove_dir_all(root).unwrap();
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
fn two_pinned_panels_keep_targets_and_back_history_independent_through_real_links() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let mut source = app.project.sources()[&entry].clone();
    source.push_str(concat!(
        "entity c kind place as \"第三资料\"\n",
        "relation_def r1 type knows from entity a to entity c\n",
        "relation_def r2 type knows from entity b to entity c\n",
    ));
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    app.project.mark_saved();
    let baseline = app.project.content_baseline();

    app.open_reading(TargetRef::new("relation", "r1"));
    click(&ctx, &mut app, 8, "钉住旁查");
    app.open_reading(TargetRef::new("relation", "r2"));
    click(&ctx, &mut app, 8, "钉住旁查");
    let ids = app.reading_panels.ids();
    let first = ids[0];
    let second = ids[1];

    let output = frame(&ctx, &mut app, Vec::new(), 8);
    let mut wide = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut wide);
    }
    assert!(wide.contains("旁查 1 · relation:r1"), "{wide}");
    assert!(wide.contains("旁查 2 · relation:r2"), "{wide}");
    click(&ctx, &mut app, 8, "实体 · 同名 · a");
    assert_eq!(
        app.reading_panels.get(first).unwrap().target,
        TargetRef::new("entity", "a")
    );
    assert_eq!(
        app.reading_panels.get(second).unwrap().target,
        TargetRef::new("relation", "r2")
    );

    click(&ctx, &mut app, 9, "实体 · 同名 · b");
    assert_eq!(
        app.reading_panels.get(second).unwrap().target,
        TargetRef::new("entity", "b")
    );
    assert_eq!(
        app.reading_panels.get(first).unwrap().target,
        TargetRef::new("entity", "a")
    );
    let output = frame(&ctx, &mut app, Vec::new(), 9);
    let mut narrow = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut narrow);
    }
    assert!(narrow.contains("旁查 2 · entity:b"), "{narrow}");
    assert!(!narrow.contains("旁查 1 · entity:a"), "{narrow}");

    app.selected_reading_panel = Some(first);
    click(&ctx, &mut app, 9, "← 返回");
    assert_eq!(
        app.reading_panels.get(first).unwrap().target,
        TargetRef::new("relation", "r1")
    );
    assert_eq!(
        app.reading_panels.get(second).unwrap().target,
        TargetRef::new("entity", "b")
    );
    app.selected_reading_panel = Some(second);
    click(&ctx, &mut app, 9, "← 返回");
    assert_eq!(
        app.reading_panels.get(second).unwrap().target,
        TargetRef::new("relation", "r2")
    );
    assert_eq!(
        app.reading_panels.get(first).unwrap().target,
        TargetRef::new("relation", "r1")
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(!app.project.is_dirty());
    assert!(app.history.is_empty());
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
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        Some(egui::Id::new(("source", &app.active_file)))
    );
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
fn opening_another_project_clears_all_personal_reading_targets() {
    let (ctx, mut app) = app();
    let original_root = app.project.root.clone();
    app.project.mark_saved();
    app.open_reading(TargetRef::new("entity", "a"));
    let first = app
        .reading_panels
        .pin(TargetRef::new("entity", "a"))
        .unwrap();
    let second = app
        .reading_panels
        .pin(TargetRef::new("entity", "b"))
        .unwrap();
    app.reading_panels
        .navigate(first, TargetRef::new("entity", "b"));
    app.selected_reading_panel = Some(second);

    let next_root = std::env::temp_dir().join(format!(
        "worldedit-reading-project-switch-{}-{}",
        std::process::id(),
        NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut next_project = Project::new(&next_root);
    let next_entry = next_project.entry.clone();
    next_project
        .set_text(&next_entry, "event fresh as \"新工程\"\n  -> END\n".into())
        .unwrap();
    next_project.save().unwrap();

    app.request_action(super::Pending::Open(next_root), &ctx);
    assert_ne!(app.project.root, original_root);
    assert!(app.reading_target.is_none());
    assert!(app.reading_history.is_empty());
    assert!(app.reading_panels.ids().is_empty());
    assert!(app.active_reading_panel.is_none());
    assert!(app.selected_reading_panel.is_none());
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .object(&TargetRef::new("event", "fresh"))
        .is_some());
}

#[test]
fn opening_another_project_is_blocked_without_dropping_pinned_context_or_form_draft() {
    let (ctx, mut app) = app();
    let original_root = app.project.root.clone();
    app.project.mark_saved();
    let panel = app
        .reading_panels
        .pin(TargetRef::new("entity", "a"))
        .unwrap();
    app.edit_entity(Some("a"));
    app.entity_editor.as_mut().unwrap().draft.description = "未提交资料旁注".into();

    let next_root = std::env::temp_dir().join(format!(
        "worldedit-reading-project-blocked-{}-{}",
        std::process::id(),
        NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut next_project = Project::new(&next_root);
    next_project.save().unwrap();

    app.request_action(super::Pending::Open(next_root), &ctx);
    assert_eq!(app.project.root, original_root);
    assert_eq!(app.reading_panels.get(panel).unwrap().target.id, "a");
    assert!(app.pending.is_none());
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.description,
        "未提交资料旁注"
    );
    assert!(app
        .message
        .as_deref()
        .is_some_and(|message| message.contains("输入已保留")));
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
fn pinned_target_refreshes_updates_and_never_falls_back_to_a_same_name_id() {
    let (ctx, mut app) = app();
    let id = app
        .reading_panels
        .pin(TargetRef::new("entity", "a"))
        .unwrap();
    let entry = app.project.entry.clone();
    app.project
        .set_text(
            &entry,
            "entity a kind place as \"更新后同名\"\nentity b kind organization as \"更新后同名\"\n"
                .into(),
        )
        .unwrap();
    app.recompile();
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 9);
    }
    let updated = frame(&ctx, &mut app, Vec::new(), 9);
    let mut rendered = String::new();
    for shape in &updated.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("更新后同名"), "{rendered}");

    app.project
        .set_text(&entry, "entity b kind place as \"更新后同名\"\n".into())
        .unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    let dirty = app.project.is_dirty();
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
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.is_dirty(), dirty);
    assert!(app.history.is_empty());
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
        .unwrap_or_else(|| {
            let mut rendered = String::new();
            for shape in &output.shapes {
                collect_text(&shape.shape, &mut rendered);
            }
            panic!("按钮未显示：{label}；当前文字：{rendered}");
        });
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
fn click_without_settling(ctx: &egui::Context, app: &mut WorldeditApp, window: u8, label: &str) {
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

fn replace_text_area(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    placeholder: &str,
    replacement: &str,
) {
    for _ in 0..3 {
        let _ = frame(ctx, app, Vec::new(), window);
    }
    let output = frame(ctx, app, Vec::new(), window);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position_contains(&shape.shape, placeholder))
        .unwrap_or_else(|| panic!("未显示可编辑文本：{placeholder}"));
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
    let key = Event::Key {
        key: egui::Key::A,
        physical_key: Some(egui::Key::A),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    };
    let release = Event::Key {
        key: egui::Key::A,
        physical_key: Some(egui::Key::A),
        pressed: false,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    };
    let _ = frame(
        ctx,
        app,
        vec![key, release, Event::Text(replacement.into())],
        window,
    );
}

fn scroll_window(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    anchor: &str,
    lines: f32,
) -> String {
    let output = frame(ctx, app, Vec::new(), window);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position_contains(&shape.shape, anchor))
        .unwrap_or_else(|| panic!("未显示滚动锚点：{anchor}"));
    let mut rendered = String::new();
    for _ in 0..12 {
        let output = frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: vec2(0.0, lines),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            window,
        );
        rendered.clear();
        for shape in &output.shapes {
            collect_text(&shape.shape, &mut rendered);
        }
    }
    rendered
}

fn drag_numeric_value(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    value_text: &str,
    horizontal_delta: f32,
) {
    let output = frame(ctx, app, Vec::new(), window);
    let start = output
        .shapes
        .iter()
        .find_map(|shape| text_position_contains(&shape.shape, value_text))
        .unwrap_or_else(|| panic!("未显示数值控件：{value_text}"));
    let _ = frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        window,
    );
    let end = start + vec2(horizontal_delta, 0.0);
    let _ = frame(ctx, app, vec![Event::PointerMoved(end)], window);
    let _ = frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(end),
            Event::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        window,
    );
}

fn click_containing(ctx: &egui::Context, app: &mut WorldeditApp, window: u8, fragment: &str) {
    let _ = scroll_to_visible(ctx, app, window, fragment, -90.0);
    for _ in 0..3 {
        let _ = frame(ctx, app, Vec::new(), window);
    }
    let output = frame(ctx, app, Vec::new(), window);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position_contains(&shape.shape, fragment))
        .unwrap_or_else(|| {
            let mut rendered = String::new();
            for shape in &output.shapes {
                collect_text(&shape.shape, &mut rendered);
            }
            panic!("按钮文字未显示：{fragment}；当前文字：{rendered}")
        });
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
fn source_mention_keyboard_commit_resolves_workspace_relative_active_source() {
    let (ctx, mut app) = app();
    let original_entry = app.project.entry.clone();
    let original_root = app.project.root.clone();
    let source = "entity a kind place as \"同名\"\nentity b kind organization as \"同名\"\nevent start\n  开始：";
    app.project
        .set_text(&original_entry, source.into())
        .unwrap();
    app.recompile();

    // Project::document resolves relative paths from CWD; keep the artwork outside this checkout.
    let cwd = std::env::current_dir().unwrap();
    let root = cwd.parent().unwrap().join(format!(
        "worldedit-relative-source-{}-{}",
        std::process::id(),
        NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    let root_name = root.file_name().unwrap().to_string_lossy().into_owned();
    let relative_entry = std::path::PathBuf::from("..")
        .join(root_name)
        .join("world.wl");
    let relocate = |path: std::path::PathBuf| {
        path.strip_prefix(&original_root)
            .ok()
            .map(std::path::Path::to_path_buf)
            .map_or(path, |relative| root.join(relative))
    };
    app.project.documents = std::mem::take(&mut app.project.documents)
        .into_iter()
        .map(|(path, document)| (relocate(path), document))
        .collect();
    app.project.authoring_documents = std::mem::take(&mut app.project.authoring_documents)
        .into_iter()
        .map(|(path, document)| (relocate(path), document))
        .collect();
    app.project.root = root.clone();
    app.project.entry = root.join("world.wl");
    app.active_file = relative_entry.clone();
    app.tab = super::Tab::Edit;
    app.recompile();

    let _ = frame(&ctx, &mut app, Vec::new(), 11);
    let editor = egui::Id::new(("source", &app.active_file));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(source.chars().count()),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let _ = frame(&ctx, &mut app, vec![Event::Text("@同名".into())], 11);

    let key = |key| Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = frame(&ctx, &mut app, vec![key(egui::Key::ArrowDown)], 11);
    let _ = frame(&ctx, &mut app, vec![key(egui::Key::Enter)], 11);

    assert!(
        app.project
            .document(&relative_entry)
            .unwrap()
            .contains("[[entity:b|同名]]"),
        "source={:?}, error={:?}",
        app.project.document(&relative_entry).unwrap(),
        app.io_error
    );
    let committed_source = app.project.document(&relative_entry).unwrap().to_owned();
    let link = "[[entity:b|同名]]";
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 11);
    }
    let output = frame(&ctx, &mut app, Vec::new(), 11);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| source_text_position(&shape.shape, &committed_source, link))
        .expect("正文引用必须在相对活动源码中可定位");
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
    assert_eq!(app.reading_target, Some(TargetRef::new("entity", "b")));
    let (return_path, return_cursor) = app.reading_return.clone().unwrap();
    assert_eq!(return_path, app.project.entry);
    click(&ctx, &mut app, 8, "返回源码编辑");
    assert_eq!(app.active_file, app.project.entry);
    assert_eq!(
        egui::TextEdit::load_state(&ctx, egui::Id::new(("source", &app.project.entry)))
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
fn project_template_manager_browses_builtin_and_project_templates_without_writing() {
    let (ctx, mut app) = app();
    register_project_template(
        &mut app,
        r#"{"schema_version":1,"id":"project:typed","title":"Typed fields","applies_to":{"kind":"entity","entity_type":"place"},"fields":[{"id":"memo","key":"memo","label":"Memo","type":"text","required":false}]}"#,
    );
    let baseline = app.project.content_baseline();
    let sources = app.project.sources().clone();
    let was_dirty = app.project.is_dirty();
    click(&ctx, &mut app, 18, "工程模板");
    assert_eq!(app.tab, super::Tab::Templates);

    let output = frame(&ctx, &mut app, Vec::new(), 17);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }

    assert!(rendered.contains("模板管理"), "{rendered}");
    assert!(rendered.contains("内置模板"), "{rendered}");
    assert!(rendered.contains("工程模板"), "{rendered}");
    assert!(rendered.contains("project:typed"), "{rendered}");
    assert!(rendered.contains("Typed fields"), "{rendered}");
    click(&ctx, &mut app, 17, "地理与地点 · template_place");
    let builtin_details = rendered_text_in_window(&ctx, &mut app, 17, "来源：内置");
    assert!(
        builtin_details.contains("来源：内置 · schema v1"),
        "{builtin_details}"
    );
    click(&ctx, &mut app, 17, "Typed fields · project:typed");
    let project_details = rendered_text_in_window(&ctx, &mut app, 17, "来源：工程");
    assert!(
        project_details.contains("来源：工程 · schema v1"),
        "{project_details}"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.sources(), sources);
    assert_eq!(app.project.is_dirty(), was_dirty);
}

#[test]
fn project_template_replacement_requires_preview_and_confirm_and_keeps_instances() {
    let (ctx, mut app) = app();
    let original_source = concat!(
        "entity a kind place as \"同名\"\n",
        "  property memo = \"保留的实例文字\"\n",
        "entity b kind place as \"同名\"\n"
    );
    app.project
        .set_text(&app.active_file.clone(), original_source.into())
        .unwrap();
    let original_template = r#"{"schema_version":1,"id":"project:typed","title":"Typed fields","applies_to":{"kind":"entity","entity_type":"place"},"fields":[{"id":"memo","key":"memo","label":"Memo","type":"text","required":false}]}"#;
    register_project_template(&mut app, original_template);
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 18, "工程模板");
    click(&ctx, &mut app, 17, "Typed fields · project:typed");

    let replacement = r#"{
  "schema_version": 1,
  "id": "project:typed",
  "title": "Typed fields",
  "applies_to": {"kind": "entity", "entity_type": "place"},
  "fields": [
    {"id": "memo", "key": "memo", "label": "Memo", "type": "number", "required": false}
  ]
}"#;
    replace_text_area(&ctx, &mut app, 17, "\"type\":\"text\"", replacement);
    click(&ctx, &mut app, 17, "预览导入 / 替换");
    let preview_text = rendered_text_in_window(&ctx, &mut app, 17, "类型变化");
    assert!(preview_text.contains("实例影响"), "{preview_text}");
    assert!(preview_text.contains("TypeMismatch"), "{preview_text}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(
        app.project.template_index().projects["project:typed"]
            .template
            .as_ref()
            .unwrap()
            .fields[0]
            .field_type,
        "text"
    );

    click(&ctx, &mut app, 17, "取消预览");
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(
        app.project.document(&app.active_file).unwrap(),
        original_source
    );

    click(&ctx, &mut app, 17, "预览导入 / 替换");
    click(&ctx, &mut app, 17, "应用预览中的模板变更");
    assert_eq!(
        app.project.template_index().projects["project:typed"]
            .template
            .as_ref()
            .unwrap()
            .fields[0]
            .field_type,
        "number"
    );
    assert!(app
        .project
        .document(&app.active_file)
        .unwrap()
        .contains("保留的实例文字"));
    assert_eq!(app.history.len(), 1);
    app.undo(false);
    assert_eq!(
        app.project.template_index().projects["project:typed"]
            .template
            .as_ref()
            .unwrap()
            .fields[0]
            .field_type,
        "text"
    );
}

#[test]
fn project_template_fields_render_by_type_and_leave_unowned_values_intact() {
    let (ctx, mut app) = app();
    let source = concat!(
        "entity a kind place as \"同名\"\n",
        "  property weight = 1.5\n",
        "  property available = false\n",
        "  property status = \"draft\"\n",
        "  property home = ref(\"entity\", \"b\")\n",
        "  property legacy_weight = \"old-format\"\n",
        "  property extension_value = \"keep me\"\n",
        "entity b kind place as \"同名\"\n",
        "entity c kind place as \"同名\"\n"
    );
    app.project
        .set_text(&app.active_file.clone(), source.into())
        .unwrap();
    register_project_template(
        &mut app,
        r#"{"schema_version":1,"id":"project:typed","title":"Typed fields","applies_to":{"kind":"entity","entity_type":"place"},"fields":[{"id":"memo","key":"memo","label":"Memo","type":"text","required":false,"default":"suggested only"},{"id":"weight","key":"weight","label":"Weight","type":"number","required":false},{"id":"available","key":"available","label":"Available","type":"boolean","required":false},{"id":"status","key":"status","label":"Status","type":"enum","required":false,"choices":["draft","ready"]},{"id":"home","key":"home","label":"Home","type":"object_ref","required":false,"target":{"kind":"entity","entity_type":"place"}},{"id":"legacy_weight","key":"legacy_weight","label":"Legacy weight","type":"number","required":false},{"id":"group","label":"Details","type":"group","fields":[{"id":"nested","key":"nested_note","label":"Nested note","type":"text","required":false}]}]}"#,
    );
    let template_doc = app.project.template_index().projects["project:typed"].clone();
    assert_eq!(
        template_doc.template.as_ref().unwrap().fields.len(),
        7,
        "{:?}",
        template_doc.diagnostics
    );
    app.edit_entity(Some("a"));
    assert!(app.entity_editor.is_some(), "{:?}", app.io_error);

    let rendered = rendered_text_in_window(&ctx, &mut app, 0, "工程模板");

    assert!(rendered.contains("工程模板 · Typed fields"), "{rendered}");
    assert!(rendered.contains("Weight · weight"), "{rendered}");
    assert!(rendered.contains("Available · available"), "{rendered}");
    assert!(rendered.contains("Status · status"), "{rendered}");
    assert!(rendered.contains("Home · home"), "{rendered}");
    drag_numeric_value(&ctx, &mut app, 0, "1.5", 10.0);
    click(&ctx, &mut app, 0, "draft");
    click(&ctx, &mut app, 0, "ready");
    click(&ctx, &mut app, 0, "Available · available");
    click(&ctx, &mut app, 0, "同名 · entity:b");
    click(&ctx, &mut app, 0, "同名 · entity:c");
    let scrolled = scroll_window(&ctx, &mut app, 0, "Home · home", -12.0);
    assert!(scrolled.contains("old-format"), "{scrolled}");
    assert!(scrolled.contains("Details"), "{scrolled}");
    click(&ctx, &mut app, 0, "应用资料");

    let entity = &app.project.compile().analysis.catalog.entities["a"];
    assert_eq!(
        entity.properties["weight"],
        worldline_core::ast::PropertyValue::Num(2.5)
    );
    assert_eq!(
        entity.properties["available"],
        worldline_core::ast::PropertyValue::Bool(true)
    );
    assert_eq!(
        entity.properties["status"],
        worldline_core::ast::PropertyValue::Str("ready".into())
    );
    assert_eq!(
        entity.properties["home"],
        worldline_core::ast::PropertyValue::Ref(TargetRef::new("entity", "c"))
    );
    assert_eq!(
        entity.properties["legacy_weight"],
        worldline_core::ast::PropertyValue::Str("old-format".into())
    );
    assert_eq!(
        entity.properties["extension_value"],
        worldline_core::ast::PropertyValue::Str("keep me".into())
    );
    assert!(!entity.properties.contains_key("memo"));
    assert!(!entity.properties.contains_key("nested_note"));
}

#[test]
fn read_only_project_template_shows_reason_and_preserves_instance_values() {
    let (ctx, mut app) = app();
    let root = app.project.root.clone();
    let source = "entity a kind place as \"同名\"\n  property memo = \"原始字段\"\n";
    let template = r#"{"schema_version":1,"id":"project:future","title":"Future template","applies_to":{"kind":"entity","entity_type":"place"},"required_features":["vendor.future.v2"],"fields":[{"id":"memo","key":"memo","label":"Memo","type":"text","required":false}]}"#;
    std::fs::create_dir_all(root.join(".world/templates")).unwrap();
    std::fs::write(root.join("world.wl"), source).unwrap();
    std::fs::write(
        root.join(".world/project.json"),
        br#"{"schema_version":1,"language_version":"1.10","entry":"world.wl","required_features":["content.entities.v1","content.templates.v1"],"maps":{},"graph_views":{},"templates":{"project:future":".world/templates/future.json"}}"#,
    )
    .unwrap();
    std::fs::write(root.join(".world/templates/future.json"), template).unwrap();
    app.project = Project::open(&root).unwrap();
    app.active_file = app.project.entry.clone();
    app.reset_views();
    app.recompile();
    let template_path = app.project.root.join(".world/templates/future.json");
    let template_bytes = app
        .project
        .authoring_document(&template_path)
        .unwrap()
        .bytes()
        .to_vec();
    assert!(app.project.template_index().projects["project:future"].read_only);

    app.edit_entity(Some("a"));
    let rendered = rendered_text_in_window(&ctx, &mut app, 0, "Future template");
    assert!(
        rendered.contains("模板格式或必需能力不受支持"),
        "{rendered}"
    );
    assert!(rendered.contains("TPL002"), "{rendered}");
    assert!(!rendered.contains("＋ Memo"), "{rendered}");
    click(&ctx, &mut app, 0, "应用资料");

    assert_eq!(
        app.project
            .authoring_document(&template_path)
            .unwrap()
            .bytes(),
        template_bytes
    );
    assert_eq!(
        app.project.compile().analysis.catalog.entities["a"].properties["memo"],
        worldline_core::ast::PropertyValue::Str("原始字段".into())
    );
}

#[test]
fn stale_project_template_preview_is_rejected_and_keeps_the_json_draft() {
    let (ctx, mut app) = app();
    let original_template = r#"{"schema_version":1,"id":"project:typed","title":"Typed fields","applies_to":{"kind":"entity","entity_type":"place"},"fields":[{"id":"memo","key":"memo","label":"Memo","type":"text","required":false}]}"#;
    register_project_template(&mut app, original_template);
    click(&ctx, &mut app, 18, "工程模板");
    click(&ctx, &mut app, 17, "Typed fields · project:typed");
    let replacement = r#"{"schema_version":1,"id":"project:typed","title":"Updated draft","applies_to":{"kind":"entity","entity_type":"place"},"fields":[{"id":"memo","key":"memo","label":"Memo","type":"text","required":false}]}"#;
    replace_text_area(
        &ctx,
        &mut app,
        17,
        "\"title\":\"Typed fields\"",
        replacement,
    );
    click(&ctx, &mut app, 17, "预览导入 / 替换");
    assert_eq!(
        app.project.template_index().projects["project:typed"]
            .template
            .as_ref()
            .unwrap()
            .title,
        "Typed fields"
    );

    let external_source = "entity a kind place as \"外部版本\"\n";
    app.project
        .set_text(&app.active_file.clone(), external_source.into())
        .unwrap();
    app.recompile();
    let external_baseline = app.project.content_baseline();
    click(&ctx, &mut app, 17, "应用预览中的模板变更");

    assert!(app
        .io_error
        .as_deref()
        .is_some_and(|error| error.contains("StaleRevision")));
    assert_eq!(app.project.content_baseline(), external_baseline);
    assert_eq!(
        app.project.document(&app.active_file).unwrap(),
        external_source
    );
    assert_eq!(
        app.project.template_index().projects["project:typed"]
            .template
            .as_ref()
            .unwrap()
            .title,
        "Typed fields"
    );
    assert!(app.history.is_empty());
    let rendered = rendered_text_in_window(&ctx, &mut app, 17, "Updated draft");
    assert!(rendered.contains("Updated draft"), "{rendered}");
}

#[test]
fn stale_project_template_form_cannot_overwrite_external_instance_edits() {
    let (ctx, mut app) = app();
    app.project
        .set_text(
            &app.active_file.clone(),
            "entity a kind place as \"同名\"\n  property weight = 1.5\n".into(),
        )
        .unwrap();
    register_project_template(
        &mut app,
        r#"{"schema_version":1,"id":"project:typed","title":"Typed fields","applies_to":{"kind":"entity","entity_type":"place"},"fields":[{"id":"weight","key":"weight","label":"Weight","type":"number","required":false}]}"#,
    );
    app.edit_entity(Some("a"));
    let rendered = rendered_text_in_window(&ctx, &mut app, 0, "Weight · weight");
    assert!(rendered.contains("Weight · weight"), "{rendered}");
    drag_numeric_value(&ctx, &mut app, 0, "1.5", 10.0);
    let draft_value = app.entity_editor.as_ref().unwrap().draft.properties[0]
        .1
        .clone();
    assert_eq!(draft_value, worldline_core::ast::PropertyValue::Num(2.5));

    let external_source =
        "entity a kind place as \"外部版本\"\n  property weight = 9.0\n  property external = \"保留\"\n";
    app.project
        .set_text(&app.active_file.clone(), external_source.into())
        .unwrap();
    app.recompile();
    let external_baseline = app.project.content_baseline();

    click(&ctx, &mut app, 0, "应用资料");

    assert_eq!(
        app.project.document(&app.active_file).unwrap(),
        external_source
    );
    assert_eq!(app.project.content_baseline(), external_baseline);
    assert!(app.history.is_empty());
    let rendered = rendered_text_in_window(&ctx, &mut app, 0, "工程已变化");
    assert!(rendered.contains("工程已变化"), "{rendered}");
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.properties[0].1,
        draft_value
    );
}

#[test]
fn copying_importing_exporting_and_deactivating_templates_keeps_instance_content() {
    let (ctx, mut app) = app();
    let source = "entity a kind place as \"同名\"\n  property extension_value = \"keep me\"\n";
    app.project
        .set_text(&app.active_file.clone(), source.into())
        .unwrap();
    app.recompile();
    let original_baseline = app.project.content_baseline();

    click(&ctx, &mut app, 18, "工程模板");
    click(&ctx, &mut app, 19, "地理与地点 · template_place");
    click(&ctx, &mut app, 19, "复制为新模板 JSON");
    click(&ctx, &mut app, 19, "预览导入 / 替换");
    let import_preview = rendered_text_in_window(&ctx, &mut app, 19, "导入影响预览");
    assert!(
        import_preview.contains("template_copy_1"),
        "{import_preview}"
    );
    let import_details = rendered_text_in_window(&ctx, &mut app, 19, "实例影响");
    assert!(
        import_details.contains("应用预览中的模板变更"),
        "{import_details}"
    );
    assert!(!app
        .project
        .template_index()
        .projects
        .contains_key("project:template_copy_1"));
    assert_eq!(app.project.content_baseline(), original_baseline);
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);

    click(&ctx, &mut app, 19, "应用预览中的模板变更");
    assert!(app
        .project
        .template_index()
        .projects
        .contains_key("project:template_copy_1"));
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    assert_ne!(app.project.content_baseline(), original_baseline);
    #[cfg(not(target_arch = "wasm32"))]
    {
        app.project.save().unwrap();
        app.project = Project::open(&app.project.root).unwrap();
        app.active_file = app.project.entry.clone();
        app.recompile();
        assert!(app
            .project
            .template_index()
            .projects
            .contains_key("project:template_copy_1"));
        assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    }

    click(
        &ctx,
        &mut app,
        19,
        "地理与地点 副本 · project:template_copy_1",
    );
    click(&ctx, &mut app, 19, "导出 JSON 到剪贴板");
    let exported = rendered_text_in_window(&ctx, &mut app, 19, "已复制到剪贴板");
    assert!(exported.contains("模板 JSON 已复制到剪贴板"), "{exported}");

    let baseline_before_deactivation = app.project.content_baseline();
    click(&ctx, &mut app, 19, "预览停用模板（保留对象资料）");
    let deactivate_header = rendered_text_in_window(&ctx, &mut app, 19, "停用影响预览");
    assert!(
        deactivate_header.contains("停用影响预览"),
        "{deactivate_header}"
    );
    let deactivate_preview = rendered_text_in_window(&ctx, &mut app, 19, "entity:a");
    assert!(
        deactivate_preview.contains("entity:a"),
        "{deactivate_preview}"
    );
    assert!(app
        .project
        .template_index()
        .projects
        .contains_key("project:template_copy_1"));
    assert_eq!(app.project.content_baseline(), baseline_before_deactivation);
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);

    click(&ctx, &mut app, 19, "取消预览");
    assert!(app
        .project
        .template_index()
        .projects
        .contains_key("project:template_copy_1"));
    click(&ctx, &mut app, 19, "预览停用模板（保留对象资料）");
    let second_deactivate_details = rendered_text_in_window(&ctx, &mut app, 19, "entity:a");
    assert!(
        second_deactivate_details.contains("确认停用模板"),
        "{second_deactivate_details}"
    );
    click(&ctx, &mut app, 19, "确认停用模板");
    assert!(!app
        .project
        .template_index()
        .projects
        .contains_key("project:template_copy_1"));
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    assert_eq!(
        app.project.compile().analysis.catalog.entities["a"].properties["extension_value"],
        worldline_core::ast::PropertyValue::Str("keep me".into())
    );
    assert_eq!(app.history.len(), 2);
    app.undo(false);
    assert!(app
        .project
        .template_index()
        .projects
        .contains_key("project:template_copy_1"));
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
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

fn scroll_rendered_text(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    needle: &str,
) -> String {
    scroll_to_visible(ctx, app, window, needle, -90.0)
}

fn scroll_window_to_top(ctx: &egui::Context, app: &mut WorldeditApp, window: u8) {
    for _ in 0..30 {
        let output = frame(ctx, app, Vec::new(), window);
        let Some(point) = detail_anchor_position(&output) else {
            continue;
        };
        let _ = frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, 360.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            window,
        );
    }
}

fn scroll_to_visible(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    needle: &str,
    delta_y: f32,
) -> String {
    let mut rendered = String::new();
    for direction in [delta_y, -delta_y] {
        for _ in 0..40 {
            let output = frame(ctx, app, Vec::new(), window);
            rendered.clear();
            for shape in &output.shapes {
                collect_text(&shape.shape, &mut rendered);
            }
            if visible_text_position(&output, needle).is_some() {
                return rendered;
            }
            let Some(point) = detail_anchor_position(&output) else {
                continue;
            };
            let _ = frame(
                ctx,
                app,
                vec![
                    Event::PointerMoved(point),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, direction),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                window,
            );
        }
    }
    rendered
}

fn detail_anchor_position(output: &egui::FullOutput) -> Option<egui::Pos2> {
    [
        "页面映射 ·",
        "同名资料提示 ·",
        "来源链接 ·",
        "附件映射 ·",
        "损失预览 ·",
        "阻塞冲突 ·",
        "写入文件预览 ·",
    ]
    .into_iter()
    .find_map(|fragment| visible_text_position(output, fragment))
}

fn visible_text_position(output: &egui::FullOutput, fragment: &str) -> Option<egui::Pos2> {
    output.shapes.iter().find_map(|clipped| {
        let point = text_position_contains(&clipped.shape, fragment)?;
        clipped.clip_rect.contains(point).then_some(point)
    })
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
fn checkpoint_history_creates_lists_previews_cancels_restores_and_undoes_through_real_ui() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let source = app.active_file.clone();
    let checkpoint_source = app.project.document(&source).unwrap().to_owned();
    let baseline = app.project.content_baseline();
    let history_len = app.history.len();

    app.tab = super::Tab::CheckpointHistory;
    app.checkpoint_history.label = "恢复测试点".into();
    click(&ctx, &mut app, 23, "创建检查点");

    let checkpoint = app
        .project
        .list_checkpoints()
        .unwrap()
        .into_iter()
        .find(|record| record.label.as_deref() == Some("恢复测试点"))
        .unwrap();
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.history.len(), history_len);
    assert_eq!(app.project.document(&source).unwrap(), checkpoint_source);
    assert_eq!(app.project.list_checkpoints().unwrap().len(), 1);

    let opened = Project::open(&source).unwrap();
    assert!(opened
        .list_checkpoints()
        .unwrap()
        .iter()
        .any(|record| record.id == checkpoint.id));

    let changed = format!("{checkpoint_source}\n// 检查点之后的本地草稿\n");
    app.project.set_text(&source, changed.clone()).unwrap();
    app.recompile();
    click(&ctx, &mut app, 23, "预览恢复…");
    let preview = frame(&ctx, &mut app, Vec::new(), 23);
    let mut preview_text = String::new();
    for shape in &preview.shapes {
        collect_text(&shape.shape, &mut preview_text);
    }
    assert!(preview_text.contains("恢复预览"), "{preview_text}");
    assert!(preview_text.contains("逐文件操作"), "{preview_text}");
    assert!(preview_text.contains("三方文本差异"), "{preview_text}");
    assert!(preview_text.contains("world.wl"), "{preview_text}");

    click(&ctx, &mut app, 23, "文本差异：world.wl");
    let expanded = frame(&ctx, &mut app, Vec::new(), 23);
    let mut expanded_text = String::new();
    for shape in &expanded.shapes {
        collect_text(&shape.shape, &mut expanded_text);
    }
    assert!(expanded_text.contains("base"), "{expanded_text}");
    assert!(expanded_text.contains("current"), "{expanded_text}");
    assert!(expanded_text.contains("checkpoint"), "{expanded_text}");
    assert!(
        expanded_text.contains("检查点之后的本地草稿"),
        "{expanded_text}"
    );

    click(&ctx, &mut app, 23, "打开恢复确认…");
    click(&ctx, &mut app, 23, "取消恢复");
    assert_eq!(app.project.document(&source).unwrap(), changed);
    assert_eq!(app.project.list_checkpoints().unwrap().len(), 1);

    click(&ctx, &mut app, 23, "预览恢复…");
    click(&ctx, &mut app, 23, "打开恢复确认…");
    assert!(app.checkpoint_history.restore_confirmation);
    click(&ctx, &mut app, 23, "确认恢复此工程检查点");
    assert_eq!(app.project.document(&source).unwrap(), checkpoint_source);
    assert!(!app.project.is_dirty());
    assert_eq!(app.history.len(), history_len + 1);

    app.undo(false);
    assert_eq!(app.project.document(&source).unwrap(), changed);
    assert!(app.project.is_dirty());
}

#[test]
fn stale_checkpoint_confirmation_is_disabled_until_a_new_preview_is_opened() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let source = app.active_file.clone();
    let checkpoint_source = app.project.document(&source).unwrap().to_owned();
    let checkpoint = app
        .project
        .create_checkpoint(
            Some("陈旧预览".into()),
            worldline_core::project::CheckpointLimits::default(),
        )
        .unwrap();

    app.tab = super::Tab::CheckpointHistory;
    app.checkpoint_history.selected_id = Some(checkpoint.id);
    let after_preview = format!("{checkpoint_source}\n// 预览时的草稿\n");
    app.project
        .set_text(&source, after_preview.clone())
        .unwrap();
    app.recompile();
    click(&ctx, &mut app, 23, "预览恢复…");
    click(&ctx, &mut app, 23, "打开恢复确认…");

    let newer_draft = format!("{after_preview}\n// 之后的草稿\n");
    app.project.set_text(&source, newer_draft.clone()).unwrap();
    app.recompile();
    let output = frame(&ctx, &mut app, Vec::new(), 23);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("此预览已过期"), "{rendered}");
    assert_eq!(app.project.document(&source).unwrap(), newer_draft);

    click(&ctx, &mut app, 23, "取消恢复");
    assert_eq!(app.project.document(&source).unwrap(), newer_draft);
    let records = app.project.list_checkpoints().unwrap();
    assert_eq!(records.len(), 1);
}

#[test]
fn escape_cancels_checkpoint_confirmation_and_small_window_keeps_history_controls_visible() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let source = app.active_file.clone();
    let original = app.project.document(&source).unwrap().to_owned();
    let checkpoint = app
        .project
        .create_checkpoint(
            Some("键盘取消".into()),
            worldline_core::project::CheckpointLimits::default(),
        )
        .unwrap();
    app.tab = super::Tab::CheckpointHistory;
    app.checkpoint_history.selected_id = Some(checkpoint.id);

    let small = frame(&ctx, &mut app, Vec::new(), 24);
    let mut rendered = String::new();
    for shape in &small.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("检查点历史"), "{rendered}");
    assert!(rendered.contains("创建检查点"), "{rendered}");
    assert!(rendered.contains("所选记录"), "{rendered}");

    let draft = format!("{original}\n// Escape 应保留的草稿\n");
    app.project.set_text(&source, draft.clone()).unwrap();
    app.recompile();
    click(&ctx, &mut app, 23, "预览恢复…");
    click(&ctx, &mut app, 23, "打开恢复确认…");
    assert!(app.checkpoint_history.restore_confirmation);
    let escape = Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = frame(&ctx, &mut app, vec![escape], 23);
    assert!(!app.checkpoint_history.restore_confirmation);
    assert!(app.checkpoint_history.preview.is_none());
    assert_eq!(app.project.document(&source).unwrap(), draft);
}

#[test]
fn checkpoint_quota_failure_keeps_label_and_project_and_delete_only_removes_history() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let source = app.active_file.clone();
    let contents = app.project.document(&source).unwrap().to_owned();
    let baseline = app.project.content_baseline();
    for _ in 0..20 {
        app.project
            .create_checkpoint(None, worldline_core::project::CheckpointLimits::default())
            .unwrap();
    }
    let history_len = app.history.len();
    app.tab = super::Tab::CheckpointHistory;
    app.checkpoint_history.label = "第 21 条".into();
    click(&ctx, &mut app, 23, "创建检查点");
    assert!(app
        .checkpoint_history
        .error
        .as_deref()
        .is_some_and(|error| error.contains("配额")));
    assert_eq!(app.checkpoint_history.label, "第 21 条");
    assert_eq!(app.project.list_checkpoints().unwrap().len(), 20);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&source).unwrap(), contents);
    assert_eq!(app.history.len(), history_len);

    let record = app.project.list_checkpoints().unwrap().remove(0);
    app.checkpoint_history.selected_id = Some(record.id.clone());
    click(&ctx, &mut app, 23, "删除历史记录…");
    let confirmation = frame(&ctx, &mut app, Vec::new(), 23);
    let mut confirmation_text = String::new();
    for shape in &confirmation.shapes {
        collect_text(&shape.shape, &mut confirmation_text);
    }
    assert!(confirmation_text.contains("不会删除或更改当前工程文件"));
    click(&ctx, &mut app, 23, "确认删除历史记录");
    assert_eq!(app.project.list_checkpoints().unwrap().len(), 19);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&source).unwrap(), contents);
    assert_eq!(app.history.len(), history_len);
}

#[test]
fn checkpoint_history_filter_and_unavailable_record_stay_read_only_and_non_executable() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let source = app.active_file.clone();
    let contents = app.project.document(&source).unwrap().to_owned();
    let baseline = app.project.content_baseline();
    let alpha = app
        .project
        .create_checkpoint(
            Some("alpha review".into()),
            worldline_core::project::CheckpointLimits::default(),
        )
        .unwrap();
    app.project
        .create_checkpoint(
            Some("beta review".into()),
            worldline_core::project::CheckpointLimits::default(),
        )
        .unwrap();
    app.tab = super::Tab::CheckpointHistory;
    app.checkpoint_history.query = "alpha".into();
    let filtered = frame(&ctx, &mut app, Vec::new(), 23);
    let mut filtered_text = String::new();
    for shape in &filtered.shapes {
        collect_text(&shape.shape, &mut filtered_text);
    }
    assert!(filtered_text.contains("alpha review"), "{filtered_text}");
    assert!(!filtered_text.contains("beta review"), "{filtered_text}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&source).unwrap(), contents);
    assert_eq!(app.project.list_checkpoints().unwrap().len(), 2);

    let payload_directory = app
        .project
        .root
        .join(".world/.checkpoints/v1")
        .join(&alpha.id)
        .join("files");
    let payload = std::fs::read_dir(payload_directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(payload, b"damaged checkpoint payload").unwrap();
    let damaged = app
        .project
        .list_checkpoints()
        .unwrap()
        .into_iter()
        .find(|record| record.id == alpha.id)
        .unwrap();
    assert!(!damaged.available);
    app.checkpoint_history.query.clear();
    app.checkpoint_history.selected_id = Some(alpha.id);
    let output = frame(&ctx, &mut app, Vec::new(), 23);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("此记录不可恢复"), "{rendered}");
    click(&ctx, &mut app, 23, "预览恢复…");
    assert!(app.checkpoint_history.preview.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&source).unwrap(), contents);
}

#[test]
fn checkpoint_confirmation_repreviews_external_workspace_changes_before_restoring() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let source = app.active_file.clone();
    let original = app.project.document(&source).unwrap().to_owned();
    let checkpoint = app
        .project
        .create_checkpoint(
            Some("磁盘变化前".into()),
            worldline_core::project::CheckpointLimits::default(),
        )
        .unwrap();
    let draft = format!("{original}\n// 保留这个工程草稿\n");
    app.project.set_text(&source, draft.clone()).unwrap();
    app.recompile();
    app.tab = super::Tab::CheckpointHistory;
    app.checkpoint_history.selected_id = Some(checkpoint.id);
    click(&ctx, &mut app, 23, "预览恢复…");
    click(&ctx, &mut app, 23, "打开恢复确认…");

    let external_file = app.project.root.join("external-note.txt");
    std::fs::write(&external_file, "外部新文件").unwrap();
    let _ = frame(&ctx, &mut app, Vec::new(), 23);
    click(&ctx, &mut app, 23, "确认恢复此工程检查点");

    assert_eq!(app.project.document(&source).unwrap(), draft);
    assert_eq!(
        std::fs::read_to_string(external_file).unwrap(),
        "外部新文件"
    );
    assert!(app.project.is_dirty());
    assert!(app.checkpoint_history.preview.is_none());
    assert!(app
        .checkpoint_history
        .error
        .as_deref()
        .is_some_and(|error| error.contains("过期")));
    assert!(app.project.list_checkpoints().unwrap()[0].available);
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
    click(&ctx, &mut app, 16, "重新比较提案");
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
            16,
        );
    }
    let output = frame(&ctx, &mut app, Vec::new(), 16);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("基底"), "{rendered}");
    assert!(rendered.contains("当前"), "{rendered}");
    assert!(rendered.contains("提议"), "{rendered}");
    assert!(rendered.contains("提议修改"), "{rendered}");
    click(&ctx, &mut app, 16, "提议");
    let output = frame(&ctx, &mut app, Vec::new(), 16);
    let mut proposed_view = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut proposed_view);
    }
    assert!(proposed_view.contains("新增地点"), "{proposed_view}");
    let history_before_apply = app.history.len();
    click(&ctx, &mut app, 16, "采纳提案");
    assert!(
        app.project.document(&entry).unwrap().contains("新增地点"),
        "{:?}",
        app.io_error
    );
    assert_eq!(app.history.len(), history_before_apply + 1);
    app.undo(false);
    assert_eq!(app.project.document(&entry).unwrap(), original);
}

fn prepare_proposal_title_conflict(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
) -> (std::path::PathBuf, String) {
    let manifest_path = app.project.root.join(".world/project.json");
    let mut manifest: serde_json::Value = serde_json::from_slice(
        app.project
            .authoring_document(&manifest_path)
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["required_features"]
        .as_array_mut()
        .unwrap()
        .push("presentation.maps.v1".into());
    manifest["maps"]["review"] = ".world/maps/review.json".into();
    app.project
        .set_authoring_document(&manifest_path, serde_json::to_vec(&manifest).unwrap())
        .unwrap();

    let map_path = app.project.root.join(".world/maps/review.json");
    let mut base = serde_json::json!({
        "schema_version": 1,
        "id": "review",
        "title": "基底",
        "raster_layers": [],
        "canvas": {"width": 1000, "height": 800, "unit": "normalized"},
        "layer_order": [],
        "layers": {},
        "placements": {},
        "extensions": {}
    });
    base["extensions"]["retired"] = "旧".into();
    let base_text = serde_json::to_string(&base).unwrap();
    app.project
        .create_authoring_document(&map_path, base_text.as_bytes().to_vec())
        .unwrap();
    app.project.save().unwrap();
    app.recompile();

    let mut proposed = base.clone();
    proposed["title"] = "提议".into();
    proposed["extensions"]
        .as_object_mut()
        .unwrap()
        .remove("retired");
    proposed["extensions"]["added"] = "新增".into();
    let proposed_text = serde_json::to_string(&proposed).unwrap();
    app.project
        .set_authoring_document(&map_path, proposed_text.as_bytes().to_vec())
        .unwrap();
    app.recompile();
    app.review.author = "审阅者".into();
    app.review.reason = "解决同字段冲突".into();
    app.review.proposal_id = "proposal_resolution".into();
    click(ctx, app, 7, "保存修改提案");

    let mut current = base;
    current["title"] = "当前".into();
    app.project
        .set_authoring_document(&map_path, serde_json::to_vec(&current).unwrap())
        .unwrap();
    app.recompile();
    click(ctx, app, 7, "重新比较提案");
    (map_path, proposed_text)
}

#[test]
fn proposal_conflict_can_be_resolved_from_a_side_and_undone() {
    let (ctx, mut app) = app();
    let (map_path, proposed_text) = prepare_proposal_title_conflict(&ctx, &mut app);
    let proposal =
        &app.snapshot.as_ref().unwrap().proposal_index.proposals["proposal_resolution"].draft;
    let preview = worldline_core::collaboration::preview_proposal(&app.project, proposal).unwrap();
    assert_eq!(preview.conflicts.len(), 1, "{:?}", preview.conflicts);
    let output = frame(&ctx, &mut app, Vec::new(), 7);
    let mut markers = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut markers);
    }
    assert!(markers.contains("当前修改 · 提议修改"), "{markers}");
    assert!(markers.contains("当前未改 · 提议新增"), "{markers}");
    assert!(markers.contains("当前未改 · 提议删除"), "{markers}");

    click(&ctx, &mut app, 7, "采纳提案");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            app.project.authoring_document(&map_path).unwrap().bytes()
        )
        .unwrap()["title"],
        "当前"
    );

    click(&ctx, &mut app, 7, "采用提议");
    let history_before_apply = app.history.len();
    click(&ctx, &mut app, 7, "采纳提案");
    let resolved: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&map_path).unwrap().bytes()).unwrap();
    assert_eq!(resolved["title"], "提议", "{:?}", app.io_error);
    assert_eq!(app.history.len(), history_before_apply + 1);
    let proposal = &app.snapshot.as_ref().unwrap().proposal_index.proposals["proposal_resolution"];
    assert_eq!(
        proposal.draft.status,
        worldline_core::collaboration::ProposalStatus::Accepted
    );
    assert_eq!(
        proposal.draft.changes[0].proposed.as_deref(),
        Some(proposed_text.as_str())
    );

    app.undo(false);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            app.project.authoring_document(&map_path).unwrap().bytes()
        )
        .unwrap()["title"],
        "当前"
    );
    assert_eq!(
        app.snapshot.as_ref().unwrap().proposal_index.proposals["proposal_resolution"]
            .draft
            .status,
        worldline_core::collaboration::ProposalStatus::Open
    );
    click(&ctx, &mut app, 7, "采纳提案");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            app.project.authoring_document(&map_path).unwrap().bytes()
        )
        .unwrap()["title"],
        "当前",
        "an accepted proposal's old resolution must not be silently reused after undo"
    );
}

#[test]
fn proposal_conflict_can_be_edited_before_core_application() {
    let (ctx, mut app) = app();
    let (map_path, _) = prepare_proposal_title_conflict(&ctx, &mut app);
    replace_text_area(&ctx, &mut app, 7, "编辑 JSON 值", "\"已解决\"");
    let mut updated_current: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&map_path).unwrap().bytes()).unwrap();
    updated_current["title"] = "当前更新".into();
    app.project
        .set_authoring_document(&map_path, serde_json::to_vec(&updated_current).unwrap())
        .unwrap();
    app.recompile();
    let output = frame(&ctx, &mut app, Vec::new(), 7);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("当前差异已过期"), "{rendered}");
    click(&ctx, &mut app, 7, "采纳提案");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            app.project.authoring_document(&map_path).unwrap().bytes()
        )
        .unwrap()["title"],
        "当前更新"
    );
    click(&ctx, &mut app, 7, "重新比较提案");
    click(&ctx, &mut app, 7, "采纳提案");
    let resolved: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&map_path).unwrap().bytes()).unwrap();
    assert_eq!(resolved["title"], "已解决", "{:?}", app.io_error);
    assert_eq!(
        app.snapshot.as_ref().unwrap().proposal_index.proposals["proposal_resolution"]
            .draft
            .status,
        worldline_core::collaboration::ProposalStatus::Accepted
    );
}

#[test]
fn reader_publish_entry_is_separate_and_explains_the_offline_boundary() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    let dirty = app.project.is_dirty();
    let destination = app
        .project
        .root
        .with_file_name(format!("reader-publish-empty-{}.zip", std::process::id()));
    let _ = std::fs::remove_file(&destination);
    click(&ctx, &mut app, 26, "发布给读者");
    replace_text_area(
        &ctx,
        &mut app,
        26,
        "reader-site.zip",
        &destination.to_string_lossy(),
    );

    click(&ctx, &mut app, 26, "生成 / 更新预览");
    let output = frame(&ctx, &mut app, Vec::new(), 26);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("离线选择不是权限认证"), "{rendered}");
    assert!(
        rendered.contains("尚未生成预览；未选任何内容时不会创建空包。"),
        "{rendered}"
    );
    assert!(
        !destination.exists(),
        "empty selection must write no output"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.is_dirty(), dirty);
}

#[test]
fn reader_publish_uses_explicit_choices_and_publishes_the_reviewed_static_zip() {
    let (ctx, mut app) = reader_publish_app();
    let baseline = app.project.content_baseline();
    let dirty = app.project.is_dirty();
    let history_len = app.history.len();
    let destination = app
        .project
        .root
        .with_file_name(format!("reader-publish-ui-{}.zip", std::process::id()));
    let _ = std::fs::remove_file(&destination);

    click(&ctx, &mut app, 26, "发布给读者");
    click(&ctx, &mut app, 26, "Public Event · event (public)");
    click(&ctx, &mut app, 26, "Public Chapter (public-chapter)");
    click(&ctx, &mut app, 26, "Public Cover (cover)");
    replace_text_area(
        &ctx,
        &mut app,
        26,
        "reader-site.zip",
        &destination.to_string_lossy(),
    );
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);

    let output = frame(&ctx, &mut app, Vec::new(), 26);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("Public Event"), "{rendered}");
    assert!(rendered.contains("Public Chapter"), "{rendered}");
    assert!(rendered.contains("Public Cover"), "{rendered}");
    assert!(rendered.contains("未公开内容"), "{rendered}");
    assert!(!destination.exists(), "preview must not write output");
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.is_dirty(), dirty);
    assert_eq!(app.history.len(), history_len);

    click(&ctx, &mut app, 26, "发布 ZIP");
    assert!(
        !destination.exists(),
        "publish must require explicit confirmation"
    );
    click(
        &ctx,
        &mut app,
        26,
        "我已逐项核对预览，确认只发布以上离线内容（不代表在线权限控制）",
    );
    click(&ctx, &mut app, 26, "发布 ZIP");
    assert!(destination.is_file());
    let zip = std::fs::read(&destination).unwrap();
    let files = crate::archive::decode(&zip).unwrap();
    assert!(files.contains_key(std::path::Path::new("index.html")));
    assert!(files.contains_key(std::path::Path::new("search-index.json")));
    assert!(files.contains_key(std::path::Path::new("search.html")));
    assert!(files.contains_key(std::path::Path::new("reader.js")));
    assert!(files.contains_key(std::path::Path::new("style.css")));
    assert!(!files.contains_key(std::path::Path::new("world.wl")));
    assert!(
        files
            .keys()
            .any(|path| path == std::path::Path::new("assets/a0001.png")),
        "{:?}",
        files.keys().collect::<Vec<_>>()
    );
    assert_reader_package_resources_resolve(&files);
    let reader_script = std::str::from_utf8(&files[std::path::Path::new("reader.js")]).unwrap();
    assert!(reader_script.contains("textContent"));
    assert!(!reader_script.contains("eval("));
    assert!(!reader_script.contains("://"));
    let emitted = files
        .iter()
        .flat_map(|(path, bytes)| {
            path.to_string_lossy()
                .bytes()
                .chain(bytes.iter().copied())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for secret in [
        "HIDDEN_EVENT_TITLE_SENTINEL",
        "HIDDEN_EVENT_BODY_SENTINEL",
        "HIDDEN_LINK_LABEL_SENTINEL",
        "HIDDEN_CHAPTER_TITLE_SENTINEL",
        "HIDDEN_CHAPTER_BODY_SENTINEL",
        "HIDDEN_ASSET_TITLE_SENTINEL",
        "HIDDEN_ATTACHMENT_BYTES_SENTINEL",
        "private",
    ] {
        assert!(
            !emitted
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()),
            "reader package leaked {secret}"
        );
    }
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.is_dirty(), dirty);
    assert_eq!(app.history.len(), history_len);
    let output = frame(&ctx, &mut app, Vec::new(), 26);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("阅读包已写入"), "{rendered}");
    let _ = std::fs::remove_file(destination);
}

#[test]
fn reader_publish_cancel_after_preview_leaves_no_output_and_preserves_author_state() {
    let (ctx, mut app) = reader_publish_app();
    let baseline = app.project.content_baseline();
    let dirty = app.project.is_dirty();
    let destination = app
        .project
        .root
        .with_file_name(format!("reader-publish-cancel-{}.zip", std::process::id()));
    let _ = std::fs::remove_file(&destination);

    click(&ctx, &mut app, 26, "发布给读者");
    click(&ctx, &mut app, 26, "Public Event · event (public)");
    replace_text_area(
        &ctx,
        &mut app,
        26,
        "reader-site.zip",
        &destination.to_string_lossy(),
    );
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);
    assert!(
        !destination.exists(),
        "preview must not write the destination"
    );
    click(&ctx, &mut app, 26, "取消发布");
    assert!(
        !destination.exists(),
        "cancel must not write the destination"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.is_dirty(), dirty);
    assert!(app.history.is_empty());
}

#[test]
fn reader_publish_recompile_refreshes_candidates_and_invalidates_the_old_review() {
    let (ctx, mut app) = reader_publish_app();
    let destination = app
        .project
        .root
        .with_file_name(format!("reader-publish-stale-{}.zip", std::process::id()));
    let _ = std::fs::remove_file(&destination);

    click(&ctx, &mut app, 26, "发布给读者");
    click(&ctx, &mut app, 26, "Public Event · event (public)");
    replace_text_area(
        &ctx,
        &mut app,
        26,
        "reader-site.zip",
        &destination.to_string_lossy(),
    );
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);
    assert!(!destination.exists());

    let entry = app.project.entry.clone();
    let mut changed = app.project.document(&entry).unwrap().to_owned();
    changed = changed.replace("Published story body.", "Updated story body.");
    app.project.set_text(&entry, changed).unwrap();
    app.recompile();
    let edited_baseline = app.project.content_baseline();
    let dirty = app.project.is_dirty();
    let output = frame(&ctx, &mut app, Vec::new(), 26);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(
        rendered.contains("尚未生成预览；未选任何内容时不会创建空包。"),
        "{rendered}"
    );
    assert!(
        rendered.contains("Public Event · event (public)"),
        "{rendered}"
    );
    assert!(!destination.exists());
    assert_eq!(app.project.content_baseline(), edited_baseline);
    assert_eq!(app.project.is_dirty(), dirty);
    assert!(app.history.is_empty());
    let _ = std::fs::remove_file(destination);
}

#[test]
fn reader_publish_cancel_and_existing_target_failure_never_write_or_overwrite() {
    let (ctx, mut app) = reader_publish_app();
    let baseline = app.project.content_baseline();
    let destination = app.project.root.with_file_name(format!(
        "reader-publish-existing-{}.zip",
        std::process::id()
    ));
    let sentinel = b"existing-output-must-survive";
    std::fs::write(&destination, sentinel).unwrap();

    click(&ctx, &mut app, 26, "发布给读者");
    click(&ctx, &mut app, 26, "Public Event · event (public)");
    replace_text_area(
        &ctx,
        &mut app,
        26,
        "reader-site.zip",
        &destination.to_string_lossy(),
    );
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);
    click(
        &ctx,
        &mut app,
        26,
        "我已逐项核对预览，确认只发布以上离线内容（不代表在线权限控制）",
    );
    click(&ctx, &mut app, 26, "发布 ZIP");
    assert_eq!(std::fs::read(&destination).unwrap(), sentinel);
    let output = frame(&ctx, &mut app, Vec::new(), 26);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("发布失败"), "{rendered}");
    assert_eq!(app.project.content_baseline(), baseline);

    click(&ctx, &mut app, 26, "取消发布");
    let output = frame(&ctx, &mut app, Vec::new(), 26);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(!rendered.contains("离线选择不是权限认证"), "{rendered}");
    assert_eq!(std::fs::read(&destination).unwrap(), sentinel);
    assert_eq!(app.project.content_baseline(), baseline);
    let _ = std::fs::remove_file(destination);
}
