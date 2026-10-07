use super::*;
use crate::app::writing_workspace::Typography;
use worldline_core::manuscript::ReviewPart;

#[test]
fn review_preserves_local_branch_boundaries_choices_speakers_and_call_scope() {
    let (ctx, mut app) = app_with_source(SOURCE);
    app.personal.settings.focus = true;
    app.manuscript.narrow_preview = true;
    let text = texts(&settle(&ctx, &mut app, vec2(900.0, 9000.0)));
    for needle in [
        "静态全分支 · 不代表真实路线",
        "条件组 · 按次序择一",
        "如果（if） · public",
        "如果（if） · rescue",
        "公开营救结局。",
        "公开留守结局。",
        "隐瞒营救结局。",
        "隐瞒留守结局。",
        "可选条件（未求值）：rescue",
        "一次性选择",
        "不可选说明：燃料不足",
        "林芜",
        "身份",
        "演出说明：低声",
        "片段调用 · 未展开",
        "call signal()",
        "仅控制流继续时汇合",
        "场景 dawn",
        "流程去向",
    ] {
        assert!(text.contains(needle), "missing {needle}: {text}");
    }
    assert!(!text.contains("片段内隐藏正文"), "调用没有偷偷展开");
    assert!(!text.contains("第二章独有正文"));
    click(&ctx, &mut app, vec2(900.0, 9000.0), "整书");
    assert!(texts(&settle(&ctx, &mut app, vec2(900.0, 9000.0))).contains("第二章独有正文"));
    assert!(app.history.is_empty());
}

#[test]
fn inline_chinese_links_share_one_layout_job_and_respect_all_typography_settings() {
    let parts = vec![
        ReviewPart {
            text: "灯塔亮起，".repeat(12),
            target: None,
            dynamic: false,
        },
        ReviewPart {
            text: "林芜https://example.invalid/a-very-long-continuous-link".into(),
            target: Some(TargetRef::new("character", "lin")),
            dynamic: false,
        },
        ReviewPart {
            text: "带着档案走过石阶。".repeat(12),
            target: None,
            dynamic: false,
        },
    ];
    let joined = parts
        .iter()
        .map(|part| part.text.as_str())
        .collect::<String>();
    let ctx = egui::Context::default();
    for size in [16.0, 28.0] {
        for spacing in [1.0, 1.45, 2.0] {
            let typography = Typography {
                source_size: 14.0,
                compact: false,
                size,
                spacing,
                width: 480.0,
            };
            let job = review_render::paragraph_job(&parts, typography, 400.0);
            assert_eq!(job.text, joined, "链接不插入空格或人工换行");
            assert_eq!(job.sections.len(), parts.len());
            assert!(job
                .sections
                .iter()
                .all(|section| section.format.font_id.size == size
                    && section.format.line_height == Some(size * spacing)));
            let _ = ctx.run(RawInput::default(), |ctx| {
                let galley = ctx.fonts(|fonts| fonts.layout_job(job.clone()));
                assert!(
                    galley.rect.width() <= 400.5,
                    "长链接应在阅读宽度内折行: {:?}",
                    galley.rect
                );
                assert!(galley.rows.len() > 2);
                for rows in galley.rows.windows(2) {
                    assert!((rows[1].pos.y - rows[0].pos.y - size * spacing).abs() < 1.0);
                }
            });
        }
    }
}

#[test]
fn minimum_window_keeps_review_actions_visible_in_both_themes_and_focus_modes() {
    for focus in [false, true] {
        for size in [16.0, 28.0] {
            for light in [false, true] {
                let (ctx, mut app) = app_with_source(SOURCE);
                app.personal.settings.focus = focus;
                app.personal.settings.body_size = size;
                app.personal.settings.line_spacing = 2.0;
                app.manuscript.narrow_preview = true;
                if light {
                    ctx.set_visuals(egui::Visuals::light());
                } else {
                    ctx.set_visuals(egui::Visuals::dark());
                }
                let output = settle(&ctx, &mut app, vec2(1040.0, 660.0));
                for label in [
                    "保存全部",
                    "阅读预览",
                    "全分支审稿",
                    "当前章节",
                    "整书",
                    "静态全分支 · 不代表真实路线",
                ] {
                    let position = visible(&output, label).unwrap_or_else(|| {
                        panic!(
                            "missing {label}, focus={focus} font={size}: {}",
                            texts(&output)
                        )
                    });
                    assert!(
                        (0.0..1040.0).contains(&position.x) && (0.0..660.0).contains(&position.y),
                        "{label}: {position:?}"
                    );
                }
                if !focus || size == 28.0 {
                    assert!(
                        visible(&output, "预览").is_some() && visible(&output, "编辑").is_some(),
                        "窄工作区或最大字号应使用可见切页入口: {}",
                        texts(&output)
                    );
                } else {
                    assert!(
                        visible(&output, "写作").is_some(),
                        "正常字号专注并排保留写作入口"
                    );
                }
                let preview = visible(&output, "全分支审稿").unwrap();
                let save = visible(&output, "保存全部").unwrap();
                assert!(preview.y > save.y + 20.0, "正文未覆盖固定主操作");
                assert!(app.history.is_empty());
            }
        }
    }
}

#[test]
fn keyboard_reaches_preview_source_and_return_without_applying() {
    let (ctx, mut app) = app_with_source(SOURCE);
    let size = vec2(1040.0, 660.0);
    app.personal.settings.focus = true;
    settle(&ctx, &mut app, size);
    let baseline = app.project.content_baseline();
    key(
        &ctx,
        &mut app,
        size,
        egui::Key::R,
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
    );
    assert!(app.manuscript.narrow_preview);
    let mut reached = false;
    for _ in 0..80 {
        let output = key(&ctx, &mut app, size, egui::Key::Tab, egui::Modifiers::NONE);
        if let Some(response) = ctx
            .memory(|memory| memory.focused())
            .and_then(|id| ctx.read_response(id))
        {
            let source_button = output.platform_output.events.iter().any(|event| matches!(event,
                egui::output::OutputEvent::FocusGained(info) if info.typ == egui::WidgetType::Button && info.label.as_deref() == Some("定位原文")));
            if source_button
                && visible(&output, "定位原文").is_some_and(|point| response.rect.contains(point))
            {
                reached = true;
                break;
            }
        }
    }
    assert!(reached, "Tab 必须抵达真实来源按钮且显示焦点");
    key(
        &ctx,
        &mut app,
        size,
        egui::Key::Enter,
        egui::Modifiers::NONE,
    );
    assert!(
        app.review_return_available(),
        "Enter未完成来源定位: notice={:?}, message={:?}, history={}, focus={:?}",
        app.manuscript.review_navigation.notice,
        app.message,
        app.personal.history.len(),
        ctx.memory(|memory| memory.focused())
    );
    assert_eq!(
        app.manuscript.writing_view.session_mode(),
        crate::app::writing_workspace::Mode::Source
    );
    key(
        &ctx,
        &mut app,
        size,
        egui::Key::ArrowLeft,
        egui::Modifiers::ALT,
    );
    settle(&ctx, &mut app, size);
    assert!(app.manuscript.narrow_preview);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn whole_book_paging_is_explicit_and_bounds_cached_chapters() {
    let (ctx, mut app) = app_with_source(SOURCE);
    let mut source = SOURCE.to_owned();
    let mut entries = vec![];
    for index in 0..21 {
        source.push_str(&format!(
            "event page{index}\n  正文第{index}章。\n  -> END\n"
        ));
        entries.push(serde_json::json!({"id":format!("chapter{index}"), "kind":"chapter", "title":format!("Chapter {index}"), "target_ref":{"kind":"event", "id":format!("page{index}")}}));
    }
    app.project
        .set_text(&app.active_file.clone(), source)
        .unwrap();
    app.project.set_authoring_document(&app.project.root.join(".world/manuscripts/book.json"), serde_json::to_vec(&serde_json::json!({"schema_version":1,"id":"book","title":"Paged Book","entries":entries})).unwrap()).unwrap();
    app.recompile();
    app.personal.settings.focus = true;
    app.manuscript.narrow_preview = true;
    app.manuscript.reader_whole_book = true;
    let size = vec2(1040.0, 660.0);
    let output = settle(&ctx, &mut app, size);
    assert!(texts(&output).contains("第 1–8 / 21 章"));
    assert!(texts(&output).contains("不代表整书已审完"));
    assert_eq!(app.manuscript.preview_cache.current.len(), 8);
    click(&ctx, &mut app, size, "下一批章节");
    let output = settle(&ctx, &mut app, size);
    assert!(texts(&output).contains("第 9–16 / 21 章"));
    assert_eq!(app.manuscript.preview_cache.current.len(), 8);
    click(&ctx, &mut app, size, "下一批章节");
    assert!(texts(&settle(&ctx, &mut app, size)).contains("第 17–21 / 21 章"));
    assert_eq!(app.manuscript.preview_cache.current.len(), 5);
    assert!(app.history.is_empty());
}

#[test]
fn page_budget_counts_each_displayed_chapter_even_if_it_reuses_same_projection() {
    let mut nodes = preview::PageBudget::default();
    assert!(nodes.admit(8_000, 500_000));
    assert!(
        !nodes.admit(8_000, 500_000),
        "同一目标重复章节也不能把节点数乘到无界"
    );
    assert!(nodes.admit(4_000, 500_000), "失败没有消耗预算");
    assert!(!nodes.admit(1, 0));
    let mut bytes = preview::PageBudget::default();
    assert!(bytes.admit(1, 4 * 1024 * 1024));
    assert!(!bytes.admit(0, 1));
    assert!(!bytes.admit(usize::MAX, usize::MAX));
}

#[test]
fn same_named_speakers_keep_distinct_identity_next_to_display_name() {
    for id in ["lin", "luo"] {
        let source = SOURCE
            .replace(
                "character lin as \"林芜\"",
                "character lin as \"林芜\"\ncharacter luo as \"林芜\"",
            )
            .replace(
                "say lin \"请记住这座城。\"",
                &format!("say {id} \"请记住这座城。\""),
            );
        let (ctx, mut app) = app_with_source(&source);
        app.personal.settings.focus = true;
        app.manuscript.narrow_preview = true;
        let size = vec2(900.0, 9000.0);
        let text = texts(&settle(&ctx, &mut app, size));
        assert!(text.contains("林芜") && text.contains("身份"));
        click(&ctx, &mut app, size, "身份");
        let text = texts(&settle(&ctx, &mut app, size));
        assert!(
            text.contains(&format!("人物 character:{id}")),
            "同名人物身份需可显式查看：{text}"
        );
    }
}

#[test]
fn focused_native_size_shows_two_endings_before_scrolling_without_shrinking_body() {
    let source = concat!(
        "let public = true\nlet rescue = false\n",
        "event start as \"天明\"\n",
        "  if public and rescue\n    林芜将档案交给等候的人，罗珀带回失踪船员。公开营救结局。\n",
        "  if public and not rescue\n    林芜公开灯塔旧案，却看着最后一艘船消失在天际。公开留守结局。\n",
        "  if not public and rescue\n    档案重新封存，失踪船员在黎明前回到港口。隐瞒营救结局。\n",
        "  if not public and not rescue\n    灯塔再次沉入海底，所有名字被留在空白的纸上。隐瞒留守结局。\n",
        "  -> END\nevent second\n  -> END\n",
    );
    let (ctx, mut app) = app_with_source(source);
    app.personal.settings.focus = true;
    app.manuscript.narrow_preview = true;
    let expected_size = app.personal.appearance().body_size;
    let output = settle(&ctx, &mut app, vec2(1188.0, 848.0));
    let mut endings = 0;
    let mut sources = 0;
    for clipped in &output.shapes {
        let mut out = Vec::new();
        text_shapes(&clipped.shape, &mut out);
        for text in out {
            let bounds = text.galley.rect.translate(text.pos.to_vec2());
            if text.pos.x > 590.0 && clipped.clip_rect.contains_rect(bounds) {
                if text.galley.job.text.ends_with("结局。") {
                    endings += 1;
                    assert!(text.galley.job.sections.iter().all(|section| section
                        .format
                        .font_id
                        .size
                        == expected_size));
                }
                if text.galley.job.text == "定位原文" {
                    sources += 1;
                }
            }
        }
    }
    assert!(
        endings >= 2,
        "1188专注首屏至少完整显示两个结局，实际{endings}：{}",
        texts(&output)
    );
    assert!(sources >= 4, "条件及正文仍有各自真实来源入口");
    assert!(
        texts(&output).matches("如果（if）").count() >= 2,
        "相邻独立if未被合成同一互斥组"
    );
}
