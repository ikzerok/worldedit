//! Tab in the code editor is indentation, not workbench focus navigation.
use super::keyboard_visibility_tests::{current_widget, key};
use super::short_viewport_tests::Harness;
use egui::{Key, Modifiers};

fn visible_caret(h: &mut Harness, id: egui::Id, phase: &str) {
    let output = h.frame(vec![]);
    // Actual editor caret geometry is exposed for IME placement; no physical IME is simulated.
    let cursor = output
        .platform_output
        .ime
        .expect("focused editable JSON exposes its caret")
        .cursor_rect;
    let response = current_widget(h, id);
    eprintln!(
        "JSON phase={phase}, raw_bytes={}, cursor={cursor:?}",
        h.app.localization_ui.exchange_json.len()
    );
    assert!(h.ctx.screen_rect().contains_rect(cursor)
        && response.interact_rect.contains_rect(cursor),
        "JSON phase={phase} must keep its real caret visible: cursor={cursor:?}, input={:?}, clipped={:?}",
        response.rect, response.interact_rect);
}

#[test]
fn localization_short_json_tab_indents_without_navigation_or_losing_visible_caret() {
    let mut h = Harness::new("event start\n  Source #wl-localization:line0\n  -> END\n");
    let entries: Vec<_> = (0..24).map(|index| format!("第 {index} 项😀")).collect();
    let mut expected = serde_json::to_string_pretty(&entries).unwrap();
    h.app.localization_ui.exchange_json = expected.clone();
    h.click("高级 JSON 交换");
    h.click(&expected);
    let id = egui::Id::new(("localization-json", &h.app.project.root, 0u64, 0usize));
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(id));
    key(
        &mut h,
        Key::End,
        Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        },
    );
    // Natural setup scrolls only the initial caret into view. No wheel is allowed in the Tab loop.
    for setup_frame in 0..30 {
        let output = h.frame(vec![]);
        let cursor = output.platform_output.ime.unwrap().cursor_rect;
        let input = current_widget(&h, id).interact_rect;
        eprintln!(
            "JSON setup={setup_frame}, cursor={cursor:?}, clipped={input:?}, wheel_point={:?}",
            h.scroll_point()
        );
        if input.contains_rect(cursor) && h.ctx.screen_rect().contains_rect(cursor) {
            break;
        }
        let delta = if cursor.top() < input.top() {
            input.top() - cursor.top()
        } else {
            input.bottom() - cursor.bottom()
        };
        h.wheel(h.scroll_point(), delta.clamp(-38.0, 38.0));
    }
    visible_caret(&mut h, id, "initial");
    let baseline = h.app.project.content_baseline();
    for tab_index in 0..16 {
        key(&mut h, Key::Tab, Modifiers::NONE);
        expected.push('\t');
        assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(id));
        assert_eq!(h.app.localization_ui.exchange_json, expected);
        visible_caret(&mut h, id, &format!("Tab {}", tab_index + 1));
    }
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h.app.history.is_empty());
    assert!(h.app.localization_ui.import_plan.is_none());
    assert!(h.app.localization_ui.has_unsubmitted_work());
}
