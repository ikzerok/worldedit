//! Full App, physical 763x542 at 200%: semantic mode actions must wrap as whole controls.
use super::*;

const ORDINARY: &str = "普通试玩";
const COMPARISON: &str = "路线对照";
const DRAFT: &str = "试演当前正文草稿…";
const RETURN_DRAFT: &str = "返回隔离试演";
const REPORT: &str = "试玩路径报告…";

fn setup(running: bool) -> Harness {
    let mut h = Harness::new();
    h.app.personal.settings.appearance.style = crate::theme::StylePreset::Studio;
    h.app.replay_debugger.locale.enabled = true;
    if running {
        h.focus_label("▶ 开始试玩", false);
        h.key(Key::Enter, Modifiers::NONE);
        assert!(h
            .app
            .play
            .as_ref()
            .is_some_and(|play| { !play.localized_outputs.is_empty() && !play.ended }));
    }
    for _ in 0..4 {
        h.frame(vec![]);
    }
    assert_eq!(h.ctx.screen_rect().size(), egui::vec2(381.5, 271.0));
    assert_eq!(h.ctx.pixels_per_point(), 2.0);
    h
}

fn tab_to(h: &mut Harness, label: &str, reverse: bool) -> egui::Id {
    let gained = h.tab(reverse);
    assert!(
        gained
            .iter()
            .any(|info| info.label.as_deref() == Some(label)),
        "expected real {}Tab to {label}, got {gained:?}",
        if reverse { "Shift+" } else { "" },
    );
    h.ctx.memory(|memory| memory.focused()).unwrap()
}

fn assert_action_visible(h: &mut Harness, label: &str) -> Rect {
    let output = h.frame(vec![]);
    assert!(!rendered(&output).contains("use of widget ID"));
    // Harness captures after App::update, inside Context::run, before end_pass swaps registries.
    // Correlate that current response with this completed output's real glyphs, not a later
    // read_response call which can silently return another pass's geometry.
    let response = h
        .focused
        .as_ref()
        .expect("focused action exists in this App frame");
    assert!(response.has_focus() && response.enabled());
    let visible = response
        .rect
        .intersect(response.interact_rect)
        .intersect(h.ctx.screen_rect());
    assert!(
        visible.is_positive()
            && visible.width() + 0.5 >= response.rect.width()
            && visible.height() + 0.5 >= response.rect.height(),
        "{label}: whole focused action must fit current clip, rect={:?}, interact={:?}, screen={:?}",
        response.rect, response.interact_rect, h.ctx.screen_rect(),
    );
    let glyphs = painted(&output, label);
    assert_eq!(
        glyphs.len(), 1,
        "{label}: a mode action must retain one readable line, not a vertical label strip: {glyphs:?}",
    );
    for (rect, clip) in glyphs {
        assert!(
            response.rect.contains_rect(rect)
                && clip.intersect(h.ctx.screen_rect()).contains_rect(rect),
            "{label}: every current-frame glyph must be inside the action and screen clip: glyph={rect:?}, clip={clip:?}, action={:?}",
            response.rect,
        );
    }
    assert!(response.rect.width() > response.rect.height());
    response.rect
}

fn unchanged(h: &Harness, before: &serde_json::Value) {
    assert_eq!(
        h.stable(),
        *before,
        "navigation must preserve real Story, RNG, trace and author inputs"
    );
    assert!(!h.app.comparison.active);
    assert!(!h.app.draft_rehearsal.active && !h.app.draft_rehearsal.has_pending());
    assert!(!h.app.playthrough_report.open);
}

#[test]
fn ordinary_mode_actions_short_200_draft_is_readable_after_real_tab_and_shift_tab() {
    for running in [false, true] {
        let mut h = setup(running);
        let before = h.stable();
        h.focus_label(ORDINARY, true);
        assert_action_visible(&mut h, ORDINARY);
        tab_to(&mut h, COMPARISON, false);
        assert_action_visible(&mut h, COMPARISON);
        let draft = tab_to(&mut h, DRAFT, false);
        assert_action_visible(&mut h, DRAFT);
        tab_to(&mut h, REPORT, false);
        assert_eq!(tab_to(&mut h, DRAFT, true), draft);
        assert_action_visible(&mut h, DRAFT);
        unchanged(&h, &before);
    }
}

#[test]
fn ordinary_mode_actions_short_200_report_fits_clip_in_both_keyboard_directions() {
    for running in [false, true] {
        let mut h = setup(running);
        let before = h.stable();
        h.focus_label(ORDINARY, true);
        tab_to(&mut h, COMPARISON, false);
        tab_to(&mut h, DRAFT, false);
        let report = tab_to(&mut h, REPORT, false);
        assert_action_visible(&mut h, REPORT);
        tab_to(&mut h, "源文", false);
        assert_eq!(tab_to(&mut h, REPORT, true), report);
        assert_action_visible(&mut h, REPORT);
        unchanged(&h, &before);
    }
}

#[test]
fn ordinary_mode_actions_wide_200_keeps_one_row_and_semantic_ids_after_resize() {
    let mut h = setup(false);
    let before = h.stable();
    h.physical = egui::vec2(3200.0, 1800.0);
    for _ in 0..4 {
        h.frame(vec![]);
    }
    let ordinary = h.focus_label(ORDINARY, false);
    let first = assert_action_visible(&mut h, ORDINARY);
    let mut actions = vec![(ORDINARY, ordinary)];
    for label in [COMPARISON, DRAFT, REPORT] {
        actions.push((label, tab_to(&mut h, label, false)));
        let rect = assert_action_visible(&mut h, label);
        assert!((rect.center().y - first.center().y).abs() < 1.0);
        assert!(rect.left() > first.right());
    }
    h.physical = egui::vec2(763.0, 542.0);
    for _ in 0..4 {
        h.frame(vec![]);
    }
    // A real navigation gesture, rather than resize itself, requests focus reveal.
    tab_to(&mut h, "源文", false);
    for (label, id) in actions.into_iter().rev() {
        assert_eq!(tab_to(&mut h, label, true), id);
        assert_action_visible(&mut h, label);
    }
    unchanged(&h, &before);
}

fn wait_for_rendered(h: &mut Harness, expected: &str) -> String {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let output = h.frame(vec![]);
        let text = rendered(&output);
        assert!(!text.contains("use of widget ID"), "{text}");
        if let Some(line) = text.lines().find(|line| line.contains(expected)) {
            return line.to_owned();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "waiting for {expected}: {text}"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

fn retained_session() -> (Harness, String) {
    let mut h = setup(true);
    h.ctx.options_mut(|options| options.warn_on_id_clash = true);
    // Preparation uses real public author inputs and real controls in a wide window.
    // Only the next isolated session uses source text; the running translated Story stays intact.
    h.physical = egui::vec2(3200.0, 1800.0);
    h.app.replay_debugger.locale.enabled = false;
    let mut buffer = h
        .app
        .project
        .open_source_writing_buffer(&h.app.project.entry)
        .unwrap();
    buffer.replace_source(SOURCE.replace("Hello", "Draft hello"));
    h.app.manuscript.restore_writing_buffers(&[buffer]);
    for _ in 0..4 {
        h.frame(vec![]);
    }
    let before = h.stable();
    h.focus_label(DRAFT, false);
    h.key(Key::Enter, Modifiers::NONE);
    assert!(h.app.draft_rehearsal.has_pending());
    wait_for_rendered(&mut h, "纳入未应用正文");
    h.focus_label("明确开始这份草稿试演", false);
    h.key(Key::Enter, Modifiers::NONE);
    assert!(h.app.draft_rehearsal.active && h.app.draft_rehearsal.has_session());
    let draft = wait_for_rendered(&mut h, "Draft hello");
    h.focus_label("保留试演，查看已应用稿", false);
    h.key(Key::Enter, Modifiers::NONE);
    assert!(!h.app.draft_rehearsal.active && h.app.draft_rehearsal.has_session());
    assert_eq!(
        h.stable(),
        before,
        "real rehearsal cannot mutate the ordinary Story"
    );
    h.app.replay_debugger.locale.enabled = true;
    h.physical = egui::vec2(763.0, 542.0);
    for _ in 0..4 {
        h.frame(vec![]);
    }
    assert_eq!(h.ctx.screen_rect().size(), egui::vec2(381.5, 271.0));
    (h, draft)
}

fn author_drafts(h: &Harness) -> Vec<(String, u64)> {
    h.app
        .manuscript
        .writing_buffers()
        .iter()
        .map(|buffer| (buffer.source().to_owned(), buffer.generation()))
        .collect()
}

fn retained_actions(h: &mut Harness) -> Vec<(&'static str, egui::Id)> {
    let ordinary = h.focus_label(ORDINARY, true);
    assert_action_visible(h, ORDINARY);
    let mut ids = vec![(ORDINARY, ordinary)];
    for label in [COMPARISON, DRAFT, RETURN_DRAFT, REPORT] {
        ids.push((label, tab_to(h, label, false)));
        assert_action_visible(h, label);
    }
    ids
}

fn confirm_no_clash(h: &mut Harness) {
    for pressed in [true, false] {
        let output = h.frame(vec![Event::Key {
            key: Key::Enter,
            physical_key: Some(Key::Enter),
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        let text = rendered(&output);
        assert!(!text.contains("use of widget ID"), "{text}");
    }
    h.frame(vec![]);
}

#[test]
fn ordinary_mode_actions_short_200_retained_real_rehearsal_stays_readable_and_returns() {
    let (mut h, draft) = retained_session();
    let before = h.stable();
    let buffers = author_drafts(&h);
    let ids = retained_actions(&mut h);
    tab_to(&mut h, "源文", false);
    for (label, id) in ids.into_iter().rev() {
        assert_eq!(tab_to(&mut h, label, true), id);
        assert_action_visible(&mut h, label);
    }
    unchanged(&h, &before);
    assert_eq!(author_drafts(&h), buffers);
    h.focus_label(RETURN_DRAFT, false);
    assert_action_visible(&mut h, RETURN_DRAFT);
    h.key(Key::Enter, Modifiers::NONE);
    assert!(h.app.draft_rehearsal.active && h.app.draft_rehearsal.has_session());
    // The short-window action already activated; compare the retained transcript in the
    // same wide reading layout used for preparation, independently of rehearsal body scrolling.
    h.physical = egui::vec2(3200.0, 1800.0);
    assert_eq!(wait_for_rendered(&mut h, "Draft hello"), draft);
    assert_eq!(h.stable(), before);
    assert_eq!(author_drafts(&h), buffers);
}

#[test]
fn ordinary_mode_actions_short_200_comparison_retained_session_keeps_clip_and_unique_ids() {
    let (mut h, _) = retained_session();
    let before = h.stable();
    let buffers = author_drafts(&h);
    let ids = retained_actions(&mut h);
    h.focus_label(COMPARISON, true);
    confirm_no_clash(&mut h);
    assert!(h.app.comparison.active);
    assert_action_visible(&mut h, COMPARISON);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(ids[1].1));
    assert_eq!(tab_to(&mut h, ORDINARY, true), ids[0].1);
    assert_action_visible(&mut h, ORDINARY);
    for &(label, id) in ids.iter().skip(1) {
        assert_eq!(tab_to(&mut h, label, false), id);
        assert_action_visible(&mut h, label);
    }
    for &(label, id) in ids.iter().rev().skip(1) {
        assert_eq!(tab_to(&mut h, label, true), id);
        assert_action_visible(&mut h, label);
    }
    assert!(h.app.comparison.active && h.app.draft_rehearsal.has_session());
    assert!(!h.app.draft_rehearsal.active && !h.app.playthrough_report.open);
    assert_eq!(h.stable(), before);
    confirm_no_clash(&mut h);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(ids[0].1));
    assert_action_visible(&mut h, ORDINARY);
    unchanged(&h, &before);
    assert_eq!(author_drafts(&h), buffers);
}
