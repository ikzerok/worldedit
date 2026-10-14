//! 实际列宽/字体与渐进菜单；旧完整 App 几何阈值另由原回归原样验证。
use super::*;

pub(super) fn visible(output: &egui::FullOutput, label: &str) -> Option<egui::Rect> {
    fn text(shape: &egui::Shape, label: &str) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.galley.rect.translate(text.pos.to_vec2()))
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| text(shape, label)),
            _ => None,
        }
    }
    output.shapes.iter().find_map(|shape| {
        text(&shape.shape, label).filter(|rect| shape.clip_rect.contains_rect(*rect))
    })
}

pub(super) fn folded(app: &mut Workbench) {
    // 通过同一真实绘制寻找 Full→Folded 的边界，不把某个字体宽度写成魔数。
    for width in (80..1500).step_by(10) {
        app.column = Some(width as f32);
        app.frame(vec![]);
        let output = app.frame(vec![]);
        if visible(&output, "对白工具").is_some() && visible(&output, "正文工具").is_none()
        {
            return;
        }
    }
    panic!("必须存在收拢新增操作而保留旧主要操作的列宽");
}

#[test]
fn dialogue_toolbar_all_styles_densities_and_body_sizes_keep_one_control_row() {
    for style in [
        theme::StylePreset::Studio,
        theme::StylePreset::Manuscript,
        theme::StylePreset::Technical,
        theme::StylePreset::Focus,
        theme::StylePreset::Ledger,
    ] {
        for density in [
            theme::Density::Compact,
            theme::Density::Standard,
            theme::Density::Spacious,
        ] {
            for body_size in [16.0, 28.0] {
                let mut app = Workbench::new("  原有旁白正文。\n");
                app.appearance = Some(theme::AppearancePreferences {
                    style,
                    density,
                    body_size,
                    reduce_motion: true,
                    ..Default::default()
                });
                for (column, height) in [
                    (1700.0, 900.0),
                    (1040.0, 660.0),
                    (588.0, 600.0),
                    (400.0, 300.0),
                ] {
                    app.size.y = height;
                    app.column = Some(column);
                    app.frame(vec![]);
                    let output = app.frame(vec![]);
                    let heading = visible(&output, "正文").unwrap();
                    let controls: &[&str] = if visible(&output, "正文工具").is_some()
                        && visible(&output, "写作").is_some()
                    {
                        &["写作", "结构", "源码", "正文工具"]
                    } else if visible(&output, "正文工具").is_some() {
                        &["正文工具", "已应用"]
                    } else if visible(&output, "对白工具").is_some() {
                        &[
                            "写作",
                            "结构",
                            "源码",
                            "应用正文草稿",
                            "丢弃此文件草稿",
                            "对白工具",
                        ]
                    } else {
                        &[
                            "写作",
                            "结构",
                            "源码",
                            "应用正文草稿",
                            "丢弃此文件草稿",
                            "插入正式台词",
                            "逐句对白",
                            "角色台本",
                        ]
                    };
                    for label in controls {
                        let rect = visible(&output, label).unwrap_or_else(|| {
                            panic!("{style:?}/{density:?}/{body_size}/{column}: {label}")
                        });
                        assert!(
                            rect.y_range().intersects(heading.y_range()),
                            "{label}不能把正文挤下一行"
                        );
                        assert!(rect.right() <= column + 8.0, "真实正文列约束");
                    }
                    assert!(
                        visible(&output, "原有旁白正文。").is_some(),
                        "首段必须完整可见"
                    );
                }
            }
        }
    }
}

#[test]
fn dialogue_toolbar_opening_folded_and_compact_menu_keeps_same_frame_text_once() {
    for compact in [false, true] {
        for mode in ["写作", "结构", "源码"] {
            for press_only in [false, true] {
                let mut app = Workbench::new("  原有旁白正文。\n");
                if mode != "写作" {
                    app.click(mode);
                }
                if compact {
                    app.size = egui::vec2(550.0, 900.0);
                } else {
                    folded(&mut app);
                }
                app.focus_body();
                let old_mode = app.view.mode;
                let old_selection = app.view.selection_mode;
                let pos = app.point(if compact {
                    "正文工具"
                } else {
                    "对白工具"
                });
                let mut events = vec![if press_only {
                    Event::Text("菜单按下输入".into())
                } else {
                    Event::Paste("菜单点击输入".into())
                }];
                events.extend(pointer(pos, true));
                if !press_only {
                    events.extend(pointer(pos, false));
                }
                app.frame(events);
                let text = if press_only {
                    "菜单按下输入"
                } else {
                    "菜单点击输入"
                };
                assert_eq!(
                    app.buffer.source().matches(text).count(),
                    1,
                    "{mode}/{compact}/{press_only}"
                );
                assert_eq!(app.view.mode, old_mode);
                assert_eq!(app.view.selection_mode, old_selection);
                assert!(app.view.mode_request.is_none());
                if press_only {
                    app.frame(pointer(pos, false));
                }
                assert!(egui::Popup::is_any_open(&app.ctx));
                app.frame(vec![]);
                assert_eq!(app.buffer.source().matches(text).count(), 1);
            }
        }
    }
}

#[test]
fn dialogue_toolbar_submenu_mode_consumes_old_input_and_closes_every_popup() {
    for compact in [false, true] {
        for typed in [false, true] {
            let mut app = Workbench::new(mixed());
            if typed {
                app.click("逐句对白");
                app.click("编辑此句");
            }
            if compact {
                app.size = egui::vec2(550.0, 900.0);
            } else {
                folded(&mut app);
            }
            app.focus_body();
            if compact {
                app.batch("正文工具", Event::Text("打开主菜单".into()));
            }
            app.batch("对白工具", Event::Text("打开对白菜单".into()));
            app.batch("逐句对白", Event::Paste("菜单内同帧输入".into()));
            assert_eq!(app.view.dialogue.enabled, !typed);
            assert!(!egui::Popup::is_any_open(&app.ctx));
            if typed {
                assert!(app.retained().contains("菜单内同帧输入"));
            } else {
                assert_eq!(app.buffer.source().matches("菜单内同帧输入").count(), 1);
            }
        }
    }
}

#[test]
fn dialogue_toolbar_submenu_ime_commit_never_switches_or_steals_the_receiver() {
    let mut app = Workbench::new(mixed());
    app.click("逐句对白");
    app.click("编辑此句");
    folded(&mut app);
    app.focus_body();
    app.batch("对白工具", Event::Text("打开时保留".into()));
    app.batch(
        "逐句对白",
        Event::Ime(ImeEvent::Commit("菜单组合保留".into())),
    );
    assert!(app.view.dialogue.enabled);
    assert!(app.retained().contains("菜单组合保留"));
    assert!(!app.buffer.is_changed());
    assert!(app.view.mode_request.is_none());
}

#[test]
fn dialogue_toolbar_production_click_receives_original_input_before_handoff() {
    for mode in ["写作", "结构", "源码", "逐句对白"] {
        for event in [
            Event::Text("交付前最后输入".into()),
            Event::Paste("交付前最后输入".into()),
        ] {
            let mut app = Workbench::new(mixed());
            if mode != "写作" {
                app.click(mode);
            }
            if mode == "逐句对白" {
                app.click("编辑此句");
            }
            folded(&mut app);
            app.focus_body();
            let original_owner = app.ctx.memory(|memory| memory.focused()).unwrap();
            let opening = if matches!(mode, "结构" | "源码") {
                "  // 开菜单时输入"
            } else {
                "开菜单时输入"
            };
            app.batch("对白工具", Event::Text(opening.into()));
            // 与 batch 相同的两个绘制帧后，交付前仍须具备有效原接收者。
            let pos = app.point("角色台本");
            assert!(
                app.project
                    .project_writing_buffer(&app.buffer, &app.target)
                    .is_ok(),
                "{mode}"
            );
            assert_eq!(
                app.ctx.memory(|memory| memory.focused()),
                Some(original_owner),
                "{mode}"
            );
            assert!(
                crate::app::writing_workspace::input_registry::owns_focus(&app.ctx),
                "{mode}"
            );
            let mut events = vec![event];
            events.extend(pointer(pos, true));
            events.extend(pointer(pos, false));
            app.frame(events);
            assert!(app.production);
            assert!(!egui::Popup::is_any_open(&app.ctx));
            if mode == "逐句对白" {
                assert!(app.retained().contains("交付前最后输入"));
                assert!(
                    app.view.has_dialogue_input(),
                    "宿主查询守卫必须仍能看到未纳入 F"
                );
                assert!(!app.buffer.is_changed());
            } else {
                assert_eq!(
                    app.buffer.source().matches("交付前最后输入").count(),
                    1,
                    "{mode}"
                );
            }
            assert!(app.view.mode_request.is_none());
        }
    }
}

#[test]
fn dialogue_toolbar_primary_and_inline_mode_press_keeps_text_before_release() {
    for label in ["写作", "结构", "源码", "逐句对白", "插入正式台词"] {
        let mut app = Workbench::new(mixed());
        app.focus_body();
        let pos = app.point(label);
        let mut events = vec![Event::Text("独立按下帧输入".into())];
        events.extend(pointer(pos, true));
        app.frame(events);
        assert_eq!(
            app.buffer.source().matches("独立按下帧输入").count(),
            1,
            "{label}"
        );
        assert_eq!(app.view.mode, Mode::Prose);
        assert!(!app.view.dialogue.enabled);
        app.frame(pointer(pos, false));
        assert_eq!(app.buffer.source().matches("独立按下帧输入").count(), 1);
    }
}

#[test]
fn dialogue_toolbar_discard_cancel_keeps_same_frame_input_and_project() {
    let mut app = Workbench::new(mixed());
    app.focus_body();
    app.frame(vec![Event::Text("已经暂存".into())]);
    app.click("丢弃此文件草稿");
    assert!(app.view.discard_confirm.is_some());
    app.focus_body();
    let baseline = app.project.content_baseline();
    app.batch("取消丢弃", Event::Paste("取消时输入".into()));
    assert!(app.view.discard_confirm.is_none());
    assert_eq!(app.buffer.source().matches("已经暂存").count(), 1);
    assert_eq!(app.buffer.source().matches("取消时输入").count(), 1);
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn dialogue_toolbar_disabled_actions_do_not_swallow_editable_body_input() {
    for label in ["应用正文草稿", "丢弃此文件草稿"] {
        for enabled in [true, false] {
            let mut app = Workbench::new(mixed());
            app.focus_body();
            app.enabled = enabled;
            let original = app.buffer.source().to_owned();
            app.batch(label, Event::Paste("禁用动作旁输入".into()));
            assert!(!app.apply);
            assert!(app.view.discard_confirm.is_none());
            if enabled {
                assert_eq!(app.buffer.source().matches("禁用动作旁输入").count(), 1);
            } else {
                assert_eq!(app.buffer.source(), original);
            }
        }
    }
}

#[test]
fn dialogue_toolbar_apply_and_discard_handoff_include_same_frame_input() {
    for label in ["应用正文草稿", "丢弃此文件草稿"] {
        let mut app = Workbench::new(mixed());
        app.allow_apply = true;
        app.focus_body();
        app.frame(vec![Event::Text("已有草稿".into())]);
        let baseline = app.project.content_baseline();
        app.batch(label, Event::Paste("动作前最后输入".into()));
        assert_eq!(app.buffer.source().matches("动作前最后输入").count(), 1);
        assert_eq!(app.apply, label == "应用正文草稿");
        assert_eq!(
            app.view.discard_confirm.is_some(),
            label == "丢弃此文件草稿"
        );
        assert_eq!(
            app.project.content_baseline(),
            baseline,
            "宿主接管前不写工程"
        );
    }
}
