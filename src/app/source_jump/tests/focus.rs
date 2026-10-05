use super::*;

#[test]
fn source_jump_task_command_and_heading_click_share_the_same_source() {
    let mut h = Harness::new(source());
    h.select(8, 2);
    let before = h.range();
    h.click("编辑");
    h.click("跳转到行列  Ctrl+G");
    h.settle();
    assert!(h.app.source_jump.open);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(query_id()));
    h.press(Key::Escape, Modifiers::NONE);
    h.settle();
    assert!(!h.app.source_jump.open);
    assert_eq!(h.range(), before);
    assert!(h.app.personal.history.is_empty());
    h.press(Key::P, Modifiers::COMMAND | Modifiers::SHIFT);
    h.settle();
    h.frame(vec![Event::Text("跳转到行列".into())]);
    h.settle();
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(h.app.source_jump.open);
    assert!(!h.app.command_palette.open);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(query_id()));
    h.press(Key::Escape, Modifiers::NONE);
    h.settle();
    assert_eq!(h.range(), before);
    let position = h.app.source_jump_position(&h.ctx).unwrap();
    h.click(&format!(
        "第 {} 行 · 第 {} 列",
        position.line, position.column
    ));
    assert!(h.app.source_jump.open);
    h.query("7:8");
    h.click("定位并收起");
    h.settle();
    assert!(!h.app.source_jump.open);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.source_id()));
    assert_eq!(h.source(), source());
}

#[test]
fn source_jump_find_and_command_layers_cancel_in_order_and_restore_valid_focus() {
    let mut h = Harness::new(source());
    h.select(8, 2);
    h.open();
    h.press(Key::F, Modifiers::COMMAND);
    h.settle();
    assert!(h.app.search_open && h.app.source_jump.open);
    assert!(h.app.edit_layer_is_top("search"));
    h.press(Key::Escape, Modifiers::NONE);
    h.settle();
    assert!(!h.app.search_open && h.app.source_jump.open);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(query_id()));
    h.press(Key::P, Modifiers::COMMAND | Modifiers::SHIFT);
    h.settle();
    assert!(h.app.edit_layer_is_top("commands"));
    h.press(Key::Escape, Modifiers::NONE);
    h.settle();
    assert!(h.app.source_jump.open && !h.app.command_palette.open);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(query_id()));
    h.press(Key::Escape, Modifiers::NONE);
    h.settle();
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.source_id()));
    assert!(h.app.personal.history.is_empty());
}

#[test]
fn source_jump_over_outline_or_find_cancel_restores_lower_layer_success_focuses_source() {
    let mut h = Harness::new(source());
    h.select(8, 2);
    for outline in [true, false] {
        if outline {
            h.press(Key::O, Modifiers::COMMAND | Modifiers::SHIFT);
        } else {
            h.press(Key::F, Modifiers::COMMAND);
        }
        h.settle();
        let original_focus = h.ctx.memory(|memory| memory.focused());
        h.open();
        assert!(h.app.edit_layer_is_top("source-jump"));
        h.press(Key::Escape, Modifiers::NONE);
        h.settle();
        assert_eq!(h.ctx.memory(|memory| memory.focused()), original_focus);
        assert_eq!(h.app.source_outline.open, outline);
        assert_eq!(h.app.search_open, !outline);
        h.open();
        h.query("7:8");
        h.press(Key::Enter, Modifiers::NONE);
        h.settle();
        assert!(!h.app.source_jump.open && !h.app.source_outline.open && !h.app.search_open);
        assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.source_id()));
        assert_eq!(h.source(), source());
    }
}

#[test]
fn source_jump_mac_control_g_does_not_steal_command_g() {
    let mut h = Harness::new(source());
    h.ctx.set_os(egui::os::OperatingSystem::Mac);
    h.select(8, 2);
    h.press(
        Key::G,
        Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        },
    );
    assert!(!h.app.source_jump.open);
    h.open();
    h.press(Key::Escape, Modifiers::NONE);
    assert!(!h.app.source_jump.open);
}
