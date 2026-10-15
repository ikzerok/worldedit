//! 正文键盘 reveal 不能把同窗标题栏或缩放边当成正文目标。
use super::*;

#[test]
fn schema_full_app_body_scroll_ignores_window_chrome_focus() {
    let mut h = FullApp::new();
    let extra = (0..6)
        .map(|index| format!("  field extra_{index} extra_{index} text\n"))
        .collect::<String>();
    let source = h
        .app
        .project
        .document(&h.app.active_file)
        .unwrap()
        .replace("character lead", &format!("{extra}character lead"));
    h.app
        .project
        .set_text(&h.app.active_file.clone(), source)
        .unwrap();
    h.app.recompile();
    h.open_from_real_character_entry();
    for _ in 0..10 {
        h.press(Key::Equals, Modifiers::COMMAND);
    }
    h.settle();
    let target = h.bind_and_fill();
    let focused_field = h.focused().id;
    let values = h.app.schema_authoring.sessions[&target].values.clone();
    let (scroll_id, viewport, content) = h.app.schema_authoring.body_scroll.unwrap();
    let initial = egui::scroll_area::State::load(&h.ctx, scroll_id)
        .unwrap()
        .offset
        .y;
    assert!(content.y - viewport.height() > initial + 100.0);
    h.frame(vec![
        Event::PointerMoved(viewport.center()),
        Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -10000.0),
            modifiers: Modifiers::NONE,
        },
    ]);
    h.settle();
    let bottom = egui::scroll_area::State::load(&h.ctx, scroll_id)
        .unwrap()
        .offset
        .y;
    assert!(bottom > initial + 100.0);
    assert!((bottom - (content.y - viewport.height())).abs() < 1.0);
    for _ in 0..20 {
        h.frame(vec![]);
    }
    assert_eq!(h.focused().id, focused_field);
    assert!(
        (egui::scroll_area::State::load(&h.ctx, scroll_id)
            .unwrap()
            .offset
            .y
            - bottom)
            .abs()
            < 1.0,
        "手动滚离输入框后不能回弹"
    );

    h.tab_to("对象来源与未应用输入", WidgetType::CollapsingHeader, false);
    h.tab_to("Close window", WidgetType::Button, false);
    let before = egui::scroll_area::State::load(&h.ctx, scroll_id)
        .unwrap()
        .offset
        .y;
    assert!(
        (before - bottom).abs() < 1.0,
        "标题关闭按钮不应改变正文滚动"
    );
    let window = h
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::new("schema-authoring-form")))
        .unwrap();
    let mut resize_ids = Vec::new();
    let mut changes = Vec::new();
    let mut reached_fixed = false;
    for step in 0..16 {
        let info = h.press(Key::Tab, Modifiers::NONE);
        h.settle();
        h.assert_sheet_focus();
        let response = h.focused();
        let offset = egui::scroll_area::State::load(&h.ctx, scroll_id)
            .unwrap()
            .offset
            .y;
        h.trace.push(format!(
            "chrome Tab {step}: id={:?} rect={:?} sense={:?} offset={offset} info={info:?}",
            response.id, response.rect, response.sense
        ));
        if (offset - before).abs() > 1.0 {
            changes.push((step, response.id, offset));
        }
        if response.sense.senses_drag()
            && (response.rect.width() <= 24.0 || response.rect.height() <= 24.0)
            && !window.shrink(12.0).contains_rect(response.rect)
            && !resize_ids.contains(&response.id)
        {
            resize_ids.push(response.id);
        }
        if info.iter().any(|info| {
            info.typ == WidgetType::Button && info.label.as_deref() == Some("预览字段修改")
        }) {
            reached_fixed = true;
            break;
        }
    }
    assert!(
        reached_fixed,
        "必须真实经过窗体控件返回固定动作：{}",
        h.trace.join("\n")
    );
    assert_eq!(
        resize_ids.len(),
        8,
        "须实际经过8个不同边/角Response，不硬编码egui私有ID：{}",
        h.trace.join("\n")
    );
    assert!(
        changes.is_empty(),
        "同窗chrome焦点改变了正文offset：before={before}, changes={changes:?}\n{}",
        h.trace.join("\n")
    );
    assert_eq!(h.app.schema_authoring.sessions[&target].values, values);
    assert!(h.app.schema_authoring.open);
}

#[test]
fn schema_full_app_body_labels_reveal_with_screen_reader_option() {
    let mut h = FullApp::new();
    let extra = (0..6)
        .map(|index| format!("  field extra_{index} extra_{index} text\n"))
        .collect::<String>();
    let source = h
        .app
        .project
        .document(&h.app.active_file)
        .unwrap()
        .replace("character lead", &format!("{extra}character lead"));
    h.app
        .project
        .set_text(&h.app.active_file.clone(), source)
        .unwrap();
    h.app.recompile();
    h.open_from_real_character_entry();
    for _ in 0..10 {
        h.press(Key::Equals, Modifiers::COMMAND);
    }
    h.settle();
    let target = h.bind_and_fill();
    let values = h.app.schema_authoring.sessions[&target].values.clone();
    // 只配置egui已支持的读屏选项；仍用真实Tab，不注入焦点或滚动状态。
    h.ctx
        .memory_mut(|memory| memory.options.screen_reader = true);
    h.settle();
    let (scroll_id, _, _) = h.app.schema_authoring.body_scroll.unwrap();
    let before = egui::scroll_area::State::load(&h.ctx, scroll_id)
        .unwrap()
        .offset
        .y;
    let label = h.tab_to("extra_5", WidgetType::Label, false);
    let response = h.focused();
    assert_eq!(response.id, label);
    let (_, viewport, _) = h.app.schema_authoring.body_scroll.unwrap();
    assert!(viewport.contains_rect(response.rect));
    assert!(response.interact_rect.contains_rect(response.rect));
    let after = egui::scroll_area::State::load(&h.ctx, scroll_id)
        .unwrap()
        .offset
        .y;
    assert!(
        after > before + 100.0,
        "可聚焦的正文Label必须实际滚入视野: {before} -> {after}"
    );
    assert_eq!(h.app.schema_authoring.sessions[&target].values, values);
    assert!(h.app.history.is_empty());
}
