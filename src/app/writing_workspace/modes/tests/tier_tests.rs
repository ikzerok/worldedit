//! Mode 优先的中间档及每次菜单重开，从真实控件/Popup状态核对。
use super::toolbar_tests::visible;
use super::*;

#[test]
fn dialogue_toolbar_modes_tier_keeps_modes_inline_and_actions_in_one_menu() {
    let mut app = Workbench::new(mixed());
    let mut found = false;
    for width in (80..1000).step_by(10) {
        app.column = Some(width as f32);
        app.frame(vec![]);
        let output = app.frame(vec![]);
        if visible(&output, "写作").is_some() && visible(&output, "正文工具").is_some() {
            found = true;
            break;
        }
    }
    assert!(found, "旧模式必须有独立于长动作标签的常驻宽度档");
    app.focus_body();
    app.batch("源码", Event::Text("常驻模式原输入".into()));
    assert_eq!(app.view.mode, Mode::Source);
    assert_eq!(app.buffer.source().matches("常驻模式原输入").count(), 1);
    app.frame(vec![]);
    app.batch("正文工具", Event::Paste("// 菜单前输入\n".into()));
    assert!(egui::Popup::is_any_open(&app.ctx));
    let output = app.frame(vec![]);
    for label in ["写作", "结构", "源码"] {
        assert_eq!(
            output
                .shapes
                .iter()
                .filter(|shape| point(&shape.shape, label).is_some())
                .count(),
            1,
            "常驻模式不能在子菜单中重复：{label}"
        );
    }
    for label in ["应用源码草稿（可含诊断）", "丢弃此文件草稿", "对白工具"] {
        assert!(visible(&output, label).is_some(), "{label}");
    }
    assert_eq!(app.buffer.source().matches("菜单前输入").count(), 1);
    app.click("对白工具");
    app.click("逐句对白");
    assert_eq!(app.view.mode, Mode::Prose);
    assert!(app.view.dialogue.enabled);
    assert!(!egui::Popup::is_any_open(&app.ctx));
}

fn tap(pos: egui::Pos2) -> Vec<Event> {
    let mut events = pointer(pos, true);
    events.extend(pointer(pos, false));
    events
}
fn escape() -> Vec<Event> {
    vec![Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]
}

#[test]
fn dialogue_toolbar_reopen_after_action_escape_or_outside_starts_at_primary_tools() {
    for closing in ["mode", "escape", "outside"] {
        let mut app = Workbench::new(mixed());
        app.size = egui::vec2(400.0, 300.0);
        if closing != "mode" {
            app.focus_body();
            app.frame(vec![Event::Text("只建立待确认草稿".into())]);
            assert!(app.buffer.is_changed());
        }
        let menu_pos = app.point("正文工具");
        let scroll_key = egui::Id::new((
            "writing-tools-menu",
            app.buffer.path(),
            &app.target.kind,
            &app.target.id,
            true,
        ))
        .with("observed-scroll");
        app.frame(tap(menu_pos));
        assert!(egui::Popup::is_any_open(&app.ctx));
        if closing == "mode" {
            let source_pos = app.point("源码");
            app.frame(tap(source_pos));
            assert_eq!(app.view.mode, Mode::Source);
        } else {
            // 确认区令短窗菜单真正溢出，避免在无需滚动的菜单上空测归顶。
            app.click("丢弃此文件草稿");
            assert!(app.view.discard_confirm.is_some());
            let menu_point = visible(&app.frame(vec![]), "结构").unwrap().center();
            app.frame(vec![
                Event::PointerMoved(menu_point),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -1200.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            for _ in 0..60 {
                app.frame(vec![]);
            }
            assert!(
                app.ctx
                    .data(|data| data.get_temp::<f32>(scroll_key))
                    .unwrap()
                    > 0.0,
                "先证明旧菜单确实滚动，重开归顶才有证据"
            );
            if closing == "escape" {
                app.frame(escape());
            } else {
                app.frame(tap(egui::pos2(390.0, 295.0)));
            }
        }
        assert!(!egui::Popup::is_any_open(&app.ctx), "{closing}");
        // 不插入空帧：紧邻下一帧重开也必须从首个旧模式入口开始。
        app.frame(tap(menu_pos));
        assert!(egui::Popup::is_any_open(&app.ctx), "{closing}");
        assert_eq!(
            app.ctx.data(|data| data.get_temp::<f32>(scroll_key)),
            Some(0.0),
            "{closing}"
        );
        // Popup 开放立即核对；Area 的首次 sizing pass 后只加一个真实绘制帧。
        let output = app.frame(vec![]);
        assert!(visible(&output, "写作").is_some(), "{closing}");
        assert!(visible(&output, "结构").is_some(), "{closing}");
    }
}

#[test]
fn dialogue_toolbar_owned_roots_reopen_immediately_and_restore_their_close_policy() {
    for compact in [false, true] {
        for multipass in [false, true] {
            for closing in ["action", "escape", "outside", "button"] {
                let mut app = Workbench::new(mixed());
                if compact {
                    app.size = egui::vec2(400.0, 300.0);
                } else {
                    super::toolbar_tests::folded(&mut app);
                }
                let label = if compact {
                    "正文工具"
                } else {
                    "对白工具"
                };
                app.focus_body();
                let position = app.point(label);
                app.force_multipass = multipass;
                let mut events = vec![if multipass {
                    Event::Text("菜单双布局输入".into())
                } else {
                    Event::Paste("菜单双布局输入".into())
                }];
                events.extend(tap(position));
                let output = app.frame(events);
                assert_eq!(app.buffer.source().matches("菜单双布局输入").count(), 1);
                assert!(egui::Popup::is_any_open(&app.ctx), "{label}/{multipass}");
                if multipass {
                    assert_eq!(output.platform_output.num_completed_passes, 2);
                }
                // 首次 Area sizing 后恰一次真实绘制，关闭表从已可见菜单开始。
                // 首开当帧的 Popup 状态/多 pass/input 断言仍在上方立即完成。
                let output = app.frame(vec![]);
                assert!(visible(&output, if compact { "写作" } else { "逐句对白" }).is_some());
                match closing {
                    "action" => {
                        let action = app.point(if compact { "源码" } else { "逐句对白" });
                        app.frame(tap(action));
                        if compact {
                            assert_eq!(app.view.mode, Mode::Source);
                        } else {
                            assert!(app.view.dialogue.enabled);
                        }
                    }
                    "escape" => {
                        app.frame(escape());
                    }
                    "outside" => {
                        let pos = app.size.to_pos2() - egui::vec2(10.0, 10.0);
                        app.frame(tap(pos));
                    }
                    "button" => {
                        app.frame(tap(position));
                    }
                    _ => unreachable!(),
                }
                assert!(
                    !egui::Popup::is_any_open(&app.ctx),
                    "{label}/{multipass}/{closing}"
                );
                // 保留真正紧邻重开，不用空绘制帧回避上一轮 Area response。
                let output = app.frame(tap(position));
                assert!(
                    egui::Popup::is_any_open(&app.ctx),
                    "{label}/{multipass}/{closing}"
                );
                if multipass {
                    assert_eq!(output.platform_output.num_completed_passes, 2);
                }
                assert_eq!(app.buffer.source().matches("菜单双布局输入").count(), 1);
                let output = app.frame(vec![]);
                assert!(visible(&output, if compact { "写作" } else { "逐句对白" }).is_some());
                // 新开适配只活在该帧，随后 Escape 与外点仍分别有效。
                app.frame(escape());
                assert!(!egui::Popup::is_any_open(&app.ctx));
                app.frame(tap(position));
                assert!(egui::Popup::is_any_open(&app.ctx));
                let output = app.frame(vec![]);
                assert!(visible(&output, if compact { "写作" } else { "逐句对白" }).is_some());
                app.frame(tap(app.size.to_pos2() - egui::vec2(10.0, 10.0)));
                assert!(!egui::Popup::is_any_open(&app.ctx));
                // 同一首开帧 Escape 仍独立关闭，IgnoreClicks 不拦截键盘取消。
                let mut events = tap(position);
                events.extend(escape());
                app.frame(events);
                assert!(!egui::Popup::is_any_open(&app.ctx));
            }
        }
    }
}
