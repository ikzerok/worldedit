//! 真实试玩/证据/重放的离屏形状回归；不替代原生字号、缩放与截图验收。
use super::{PlayPane, WorldeditApp};
use crate::theme::{self, ThemeMode};
use egui::{Color32, Event, Pos2, Rect};
use worldline_core::project::Project;
use worldline_runtime::{ReplayBudget, ReplayCancellation, ReplayTrace};

#[derive(Clone, Copy)]
struct Appearance {
    mode: ThemeMode,
    system: Option<egui::Theme>,
}

fn appearances() -> [Appearance; 4] {
    [
        Appearance {
            mode: ThemeMode::Dark,
            system: Some(egui::Theme::Light),
        },
        Appearance {
            mode: ThemeMode::Light,
            system: Some(egui::Theme::Dark),
        },
        Appearance {
            mode: ThemeMode::System,
            system: Some(egui::Theme::Dark),
        },
        Appearance {
            mode: ThemeMode::System,
            system: Some(egui::Theme::Light),
        },
    ]
}

fn fixture(source: &str) -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = std::env::temp_dir().join(format!(
        "worldedit-play-colors-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.project = Project::new(&root);
    let entry = app.project.entry.clone();
    app.project.documents.retain(|path, _| path == &entry);
    app.project.set_text(&entry, source.into()).unwrap();
    app.project
        .create_authoring_document(
            &root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.13","required_features":["content.choice_presentation.v1"]}"#.to_vec(),
        )
        .unwrap();
    app.active_file = entry;
    app.reset_views();
    app.recompile();
    (ctx, app)
}

fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    appearance: Appearance,
    width: f32,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(width, 3000.0))),
            system_theme: appearance.system,
            events,
            ..Default::default()
        },
        |ctx| {
            theme::configure(ctx, appearance.mode);
            ctx.style_mut(|style| style.animation_time = 0.0);
            app.play_tab(ctx);
        },
    )
}

fn settled(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    appearance: Appearance,
    width: f32,
) -> egui::FullOutput {
    for _ in 0..3 {
        let _ = frame(ctx, app, appearance, width, vec![Event::PointerGone]);
    }
    frame(ctx, app, appearance, width, vec![Event::PointerGone])
}

fn start(app: &mut WorldeditApp) {
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.snapshot.as_ref().unwrap().result.diagnostics
    );
    app.start_play();
    assert!(app.play.is_some(), "测试稿没有未应用输入，应直接开始试玩");
}

fn flattened<'a>(shape: &'a egui::Shape, clip: Rect, out: &mut Vec<(Rect, &'a egui::Shape)>) {
    if let egui::Shape::Vec(shapes) = shape {
        for shape in shapes {
            flattened(shape, clip, out);
        }
    } else {
        out.push((clip, shape));
    }
}

fn shapes(output: &egui::FullOutput) -> Vec<(Rect, &egui::Shape)> {
    let mut shapes = Vec::new();
    for shape in &output.shapes {
        flattened(&shape.shape, shape.clip_rect, &mut shapes);
    }
    shapes
}

fn visible_texts(output: &egui::FullOutput) -> String {
    shapes(output)
        .into_iter()
        .filter_map(|(clip, shape)| match shape {
            egui::Shape::Text(text) if clip.intersects(text.visual_bounding_rect()) => {
                Some(text.galley.text())
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn contrast(a: Color32, b: Color32) -> f32 {
    let luminance = |color: Color32| {
        let channel = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// 仅取文字之前的矩形填充，按绘制顺序合成；文字、边框和后绘制项不能充当背景。
fn background_at(shapes: &[(Rect, &egui::Shape)], point: Pos2) -> Color32 {
    let mut background = Color32::TRANSPARENT;
    for (clip, shape) in shapes {
        if let egui::Shape::Rect(rect) = shape {
            if clip.contains(point) && rect.rect.contains(point) {
                background = background.blend(rect.fill);
            }
        }
    }
    assert_eq!(background.a(), 255, "可见文字下必须有实际绘制的不透明背景");
    background
}

/// 逐行核对实际裁剪内的文字，不把窗口外的形状算作可见结果。
fn assert_readable(output: &egui::FullOutput, fragment: &str, expected: Color32) {
    let shapes = shapes(output);
    let mut found = 0;
    for (index, (clip, shape)) in shapes.iter().enumerate() {
        let egui::Shape::Text(text) = shape else {
            continue;
        };
        if !text.galley.text().contains(fragment) {
            continue;
        }
        for row in &text.galley.rows {
            let visible = clip.intersect(row.rect().translate(text.pos.to_vec2()));
            if row.glyphs.is_empty() || !visible.is_positive() {
                continue;
            }
            let background = background_at(&shapes[..index], visible.center());
            for section in &text.galley.job.sections {
                let color = text.override_text_color.unwrap_or_else(|| {
                    if section.format.color == Color32::PLACEHOLDER {
                        text.fallback_color
                    } else {
                        section.format.color
                    }
                });
                assert_eq!(color, expected, "语义色未接入：{}", text.galley.text());
                let composited = background.blend(color.gamma_multiply(text.opacity_factor));
                let ratio = contrast(composited, background);
                assert!(
                    ratio >= 4.5,
                    "{fragment}: {composited:?} on {background:?}, contrast {ratio:.2}:1"
                );
            }
            found += 1;
        }
    }
    assert!(
        found > 0,
        "未实际绘制可见文字 {fragment}：\n{}",
        visible_texts(output)
    );
}

fn click(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    appearance: Appearance,
    width: f32,
    label: &str,
) {
    let output = settled(ctx, app, appearance, width);
    let point = shapes(&output)
        .into_iter()
        .find_map(|(clip, shape)| match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let visible = clip.intersect(text.visual_bounding_rect());
                visible.is_positive().then(|| visible.center())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("未显示按钮 {label}：{}", visible_texts(&output)));
    for pressed in [true, false] {
        let _ = frame(
            ctx,
            app,
            appearance,
            width,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
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
fn play_compile_error_is_readable_in_dark_light_and_system() {
    for appearance in appearances() {
        let (ctx, mut app) = fixture("event start\n  -> missing_destination\n");
        assert!(app.snapshot.as_ref().unwrap().result.has_errors());
        let output = settled(&ctx, &mut app, appearance, 1600.0);
        assert_readable(&output, "故事存在错误", theme::ERROR());
        assert!(app.play.is_none());
    }
}

#[test]
fn play_end_and_anchor_keep_readable_textual_semantics_in_every_theme() {
    for appearance in appearances() {
        let (ctx, mut app) = fixture(concat!(
            "tag calm\ntag alert\ncharacter lin\nstate mood on character lin with calm\n",
            "event start\n",
            "  become mood with alert as \"状态变更说明\"\n",
            "  anchor \"存档点\" as \"核对锚点说明\"\n",
            "  收束正文。\n  -> END\n",
        ));
        start(&mut app);
        let output = settled(&ctx, &mut app, appearance, 1600.0);
        assert!(app.play.as_ref().unwrap().ended);
        assert_readable(&output, "世界线收束", theme::SUCCESS());
        assert_readable(&output, "◆ [", theme::ANCHOR());
        assert_readable(&output, "↳ 核对锚点说明", theme::MUTED());
        assert_readable(&output, "@start · 故事线", theme::MUTED());
        click(&ctx, &mut app, appearance, 1600.0, "状态变更记录 · 1");
        let output = settled(&ctx, &mut app, appearance, 1600.0);
        assert_readable(&output, "start · 轮次 0", theme::MUTED());
    }
}

#[test]
fn play_runtime_error_stale_warning_and_debugger_notice_use_semantic_tokens() {
    for appearance in appearances() {
        let (ctx, mut app) = fixture(concat!(
            "event start\n",
            "  choice \"故障选项\" if 1 / 0 > 0\n    -> END\n",
        ));
        start(&mut app);
        let _ = settled(&ctx, &mut app, appearance, 1600.0);
        assert!(app.play.as_ref().unwrap().error.is_some());
        app.version += 1;
        app.replay_debugger.notice = Some("测试通知：路径未能导入".into());
        let output = settled(&ctx, &mut app, appearance, 1600.0);
        assert_readable(&output, "运行错误:", theme::ERROR());
        assert_readable(&output, "当前结果仍属于旧快照", theme::WARNING());
        assert_readable(&output, "测试通知：路径未能导入", theme::WARNING());
        assert_readable(&output, "试玩已暂停", theme::MUTED());
        assert_readable(&output, "(暂无;", theme::MUTED());
        app.replay_debugger.pane = PlayPane::Debugger;
        let output = settled(&ctx, &mut app, appearance, 760.0);
        assert_readable(&output, "测试通知：路径未能导入", theme::WARNING());
    }
}

#[test]
fn debugger_import_error_and_stale_result_are_readable_on_real_surfaces() {
    for appearance in appearances() {
        let (ctx, mut app) = fixture("event start\n  choice \"继续\"\n    -> END\n");
        start(&mut app);
        let _ = settled(&ctx, &mut app, appearance, 1600.0);
        app.replay_debugger.import_json =
            "x".repeat(worldline_runtime::MAX_REPLAY_EXCHANGE_BYTES + 1);
        click(
            &ctx,
            &mut app,
            appearance,
            1600.0,
            "导入路径 JSON（最多 4 MiB / 20,000 步）",
        );
        let output = settled(&ctx, &mut app, appearance, 1600.0);
        assert_readable(&output, "内容已拒绝导入", theme::ERROR());
        let trace = app
            .play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .replay_trace();
        let snapshot = app.snapshot.as_ref().unwrap();
        app.replay_debugger.result = Some(
            ReplayTrace::replay(
                &snapshot.result.program,
                &snapshot.result.analysis,
                &trace,
                ReplayBudget::default(),
                &ReplayCancellation::new(),
            )
            .unwrap(),
        );
        app.replay_debugger.result_version = Some(app.version);
        app.version += 1;
        let output = settled(&ctx, &mut app, appearance, 1600.0);
        assert_readable(&output, "此结果属于旧编译快照", theme::WARNING());
    }
}

#[test]
fn evidence_error_and_not_evaluated_are_readable_without_recoloring_truth_values() {
    for appearance in appearances() {
        let (ctx, mut app) = fixture(concat!(
            "event start\n",
            "  choice \"失败条件\" if (1 / 0 > 0) and (1 > 0)\n    -> END\n",
        ));
        start(&mut app);
        click(&ctx, &mut app, appearance, 1600.0, "解释当前条件（只读）");
        let before = app
            .play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .save()
            .unwrap();
        let output = settled(&ctx, &mut app, appearance, 1600.0);
        assert_readable(&output, "→ 求值错误", theme::ERROR());
        assert_readable(&output, "→ 未求值", theme::MUTED());
        assert_eq!(
            app.play
                .as_ref()
                .unwrap()
                .story
                .as_ref()
                .unwrap()
                .save()
                .unwrap(),
            before
        );

        let (ctx, mut app) = fixture(concat!(
            "event start\n",
            "  choice \"真假解释\" if not (true or false)\n    -> END\n",
            "  choice \"继续\"\n    -> END\n",
        ));
        start(&mut app);
        click(&ctx, &mut app, appearance, 1600.0, "解释当前条件（只读）");
        let output = settled(&ctx, &mut app, appearance, 1600.0);
        assert_readable(&output, "→ 已满足 · true", theme::TEXT());
        assert_readable(&output, "→ 未满足 · false", theme::TEXT());
    }
}

#[test]
fn evidence_omitted_values_and_label_errors_remain_readable() {
    for appearance in appearances() {
        let source = format!(
            "let long = \"{}\"\nevent start\n  choice \"长值\" if long == \"\"\n    -> END\n  choice \"继续\"\n    -> END\n",
            "x".repeat(20_000)
        );
        let (ctx, mut app) = fixture(&source);
        start(&mut app);
        click(&ctx, &mut app, appearance, 1600.0, "解释当前条件（只读）");
        let output = settled(&ctx, &mut app, appearance, 1600.0);
        assert_readable(&output, "→ 证据已省略", theme::MUTED());
        assert_readable(&output, "部分证据已省略", theme::MUTED());

        let (ctx, mut app) =
            fixture("event start\n  choice \"标签 {1 / 0}\" if true\n    -> END\n");
        start(&mut app);
        click(&ctx, &mut app, appearance, 1600.0, "解释当前条件（只读）");
        let output = settled(&ctx, &mut app, appearance, 1600.0);
        assert_readable(&output, "选择标签求值失败", theme::ERROR());
        assert_readable(&output, "→ 已满足 · true", theme::TEXT());
    }
}

#[test]
fn disabled_choice_is_an_inactive_exception_with_readable_reason_and_no_activation() {
    for appearance in appearances() {
        let (ctx, mut app) = fixture(concat!(
            "event start\n",
            "  choice \"锁定\" enable false disabled \"缺少钥匙\"\n    -> END\n",
            "  choice \"继续\"\n    -> END\n",
        ));
        start(&mut app);
        let output = settled(&ctx, &mut app, appearance, 1600.0);
        // inactive 控件本身单列例外；其可见原因是普通文字，仍必须达到4.5:1。
        assert!(visible_texts(&output).contains("选择：锁定"));
        assert_readable(&output, "暂不可选：缺少钥匙", theme::MUTED());
        let story = app.play.as_ref().unwrap().story.as_ref().unwrap();
        assert!(!story.choice_presentations()[0].enabled);
        let before = (story.save().unwrap(), story.replay_trace());
        click(&ctx, &mut app, appearance, 1600.0, "选择：锁定");
        let story = app.play.as_ref().unwrap().story.as_ref().unwrap();
        assert_eq!((story.save().unwrap(), story.replay_trace()), before);
        assert!(!app.play.as_ref().unwrap().ended);
    }
}
