//! Real App keyboard navigation at native 763×541 / 200%; pointer use is only a starting action.
use super::short_viewport_tests::Harness;
use super::workbench_tests::stage;
use super::*;
use egui::{Event, Key, Modifiers};
mod delivery_tests;

const SOURCE: &str = "event start\n  Source #wl-localization:line0\n  -> END\n";

fn focused(h: &Harness) -> egui::Id {
    h.ctx
        .memory(|memory| memory.focused())
        .expect("real key navigation keeps focus")
}

pub(super) fn current_widget(h: &Harness, id: egui::Id) -> egui::WidgetRect {
    h.ctx
        .viewport(|viewport| viewport.prev_pass.widgets.get(id).copied())
        .expect("control was actually registered in the completed frame")
}

#[track_caller]
fn complete(h: &Harness, id: egui::Id, label: &str) {
    let response = current_widget(h, id);
    assert_eq!(focused(h), id, "{label} owns the real focus");
    assert!(
        h.ctx.screen_rect().contains_rect(response.rect)
            && response.interact_rect.contains_rect(response.rect),
        "{label} must be wholly visible after keyboard navigation without wheel/pointer help: rect={:?}, interact={:?}, screen={:?}, frames={:#?}",
        response.rect, response.interact_rect, h.ctx.screen_rect(), h.frame_trace
    );
}

#[track_caller]
pub(super) fn key(
    h: &mut Harness,
    key: Key,
    modifiers: Modifiers,
) -> Vec<egui::output::OutputEvent> {
    if h.trace_frames {
        h.record_trace(format!(
            "key {key:?} {modifiers:?} from {}",
            std::panic::Location::caller()
        ));
    }
    let mut events = Vec::new();
    for pressed in [true, false] {
        events.extend(
            h.frame(vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            }])
            .platform_output
            .events,
        );
    }
    // Shift+Tab can transfer focus on the following frame; never move it in the harness.
    for _ in 0..6 {
        events.extend(h.frame(vec![]).platform_output.events);
    }
    events
}

#[track_caller]
fn tab(h: &mut Harness, label: &str, reverse: bool) -> egui::Id {
    let events = key(
        h,
        Key::Tab,
        if reverse {
            Modifiers::SHIFT
        } else {
            Modifiers::NONE
        },
    );
    let id = focused(h);
    complete(h, id, label);
    assert!(
        events.iter().any(|event| matches!(event,
        egui::output::OutputEvent::FocusGained(info) if info.label.as_deref() == Some(label))),
        "{}Tab must reach {label}; focused={id:?}; events={events:?}",
        if reverse { "Shift+" } else { "" }
    );
    id
}

#[track_caller]
fn tab_field(h: &mut Harness, id: egui::Id, label: &str, reverse: bool) {
    key(
        h,
        Key::Tab,
        if reverse {
            Modifiers::SHIFT
        } else {
            Modifiers::NONE
        },
    );
    assert_eq!(
        focused(h),
        id,
        "{label} must retain its actual field identity"
    );
    complete(h, id, label);
}

fn typed_detail() -> (Harness, egui::Id) {
    let mut h = Harness::new(SOURCE);
    stage(&mut h.app.localization_ui, "Original translation");
    h.click("当前源文与译文");
    let draft_key = editing::draft_key("zh-Hant", "line0");
    let id = egui::Id::new((
        "localization-part-text",
        &h.app.project.root,
        &draft_key,
        0usize,
    ));
    // Natural pointer setup makes the entire initial field visible, not just its first text row.
    h.click_field(id, "Original translation");
    assert_eq!(focused(&h), id);
    key(
        &mut h,
        Key::A,
        Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        },
    );
    for character in "HELLO中文😀".chars() {
        h.frame(vec![Event::Text(character.to_string())]);
        h.frame(vec![]);
        assert_eq!(focused(&h), id);
    }
    assert_eq!(
        h.app.localization_ui.workbench.drafts[&draft_key]
            .edit
            .translation_parts,
        [LocalizationPart::Text {
            text: "HELLO中文😀".into()
        }]
    );
    complete(&h, id, "typed translation before Tab");
    (h, id)
}

fn assert_typed_unchanged(h: &Harness, baseline: &str, history: usize) {
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert_eq!(h.app.history.len(), history);
    assert_eq!(
        h.app.localization_ui.workbench.drafts[&editing::draft_key("zh-Hant", "line0")]
            .edit
            .translation_parts,
        [LocalizationPart::Text {
            text: "HELLO中文😀".into()
        }]
    );
    assert!(h.app.localization_ui.has_unsubmitted_work());
    assert!(h.app.localization_ui.workbench.preview.is_none());
}

#[test]
fn localization_short_keyboard_typed_detail_tab_reveals_actions_and_shift_tab_returns() {
    let (mut h, field) = typed_detail();
    let baseline = h.app.project.content_baseline();
    let history = h.app.history.len();
    let add = tab(&mut h, "添加文字段", false);
    tab(&mut h, "保留此译文并待复核", false);
    assert_eq!(tab(&mut h, "添加文字段", true), add);
    tab_field(&mut h, field, "Shift+Tab translation", true);
    assert_typed_unchanged(&h, &baseline, history);
}

#[test]
fn localization_short_keyboard_shift_tab_from_visible_detail_action_reveals_translation() {
    let (mut h, field) = typed_detail();
    // Mouse clicks need not grant keyboard focus. Reach the reverse starting point with real Tab.
    tab(&mut h, "添加文字段", false);
    tab(&mut h, "保留此译文并待复核", false);
    let baseline = h.app.project.content_baseline();
    let history = h.app.history.len();
    complete(&h, focused(&h), "keyboard-focused keep action");
    tab(&mut h, "添加文字段", true);
    tab_field(&mut h, field, "Shift+Tab translation from bottom", true);
    assert_typed_unchanged(&h, &baseline, history);
}

#[test]
fn localization_short_keyboard_directory_controls_stay_visible_in_both_directions() {
    let mut h = Harness::new(SOURCE);
    let baseline = h.app.project.content_baseline();
    h.click("en");
    let target = egui::Id::new(("localization-target-locale", &h.app.project.root, false));
    tab_field(&mut h, target, "target locale", false);
    let refresh = tab(&mut h, "刷新目录", false);
    let directory = tab(&mut h, "字符串目录", false);
    let detail = tab(&mut h, "当前源文与译文", false);
    let search = egui::Id::new(("localization-search", &h.app.project.root));
    tab_field(&mut h, search, "catalog search", false);
    tab_field(&mut h, detail, "detail tab", true);
    tab_field(&mut h, directory, "directory tab", true);
    tab_field(&mut h, refresh, "refresh", true);
    tab_field(&mut h, target, "target locale", true);
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h.app.history.is_empty());
    assert!(!h.app.localization_ui.has_unsubmitted_work());
}

#[test]
fn localization_short_keyboard_advanced_actions_stay_visible_and_keep_whitelist_and_json() {
    advanced_actions(Harness::new(SOURCE));
}

fn advanced_actions(mut h: Harness) {
    h.trace_frames = true;
    // Pointer/wheel setup keeps the existing harness contract; only keyboard delivery varies.
    let keyboard_seconds = h.frame_seconds;
    h.frame_seconds = 1.0 / 60.0;
    h.app.localization_ui.string_ids = "line0".into();
    h.app.localization_ui.exchange_json = "{完整待核对 JSON😀}".into();
    h.click("高级 JSON 交换");
    h.click("line0");
    h.frame_seconds = keyboard_seconds;
    let whitelist = egui::Id::new(("localization-whitelist", &h.app.project.root));
    assert_eq!(focused(&h), whitelist);
    let baseline = h.app.project.content_baseline();
    h.record_trace("advanced phase: first whitelist to export preview".into());
    let preview = tab(&mut h, "预览导出", false);
    let paths = tab(&mut h, "明确文件路径", false);
    let choose = tab(&mut h, "选择 JSON 交换文件…", false);
    tab(&mut h, "预览导入", false);
    tab_field(&mut h, choose, "choose JSON", true);
    tab_field(&mut h, paths, "file paths", true);
    tab_field(&mut h, preview, "export preview", true);
    tab_field(&mut h, whitelist, "whitelist", true);
    h.record_trace("advanced phase: returned whitelist to export preview".into());
    assert_eq!(tab(&mut h, "预览导出", false), preview);
    key(&mut h, Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(
        h.app
            .localization_ui
            .export_plan
            .as_ref()
            .unwrap()
            .can_export
    );
    assert_eq!(
        focused(&h),
        preview,
        "pending notice and accepted plan must not renumber the preview action"
    );
    complete(&h, preview, "preview action after accepted plan");
    tab(&mut h, "导出 UTF-8 JSON…", false);
    h.record_trace("advanced phase: accepted export action back to preview".into());
    assert_eq!(tab(&mut h, "预览导出", true), preview);
    assert_eq!(h.app.localization_ui.string_ids, "line0");
    assert_eq!(h.app.localization_ui.exchange_json, "{完整待核对 JSON😀}");
    assert!(h.app.localization_ui.import_plan.is_none());
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h.app.history.is_empty());
}

pub(super) fn describe_frame(h: &Harness, events: &str) -> String {
    // Exact egui 0.32 CentralPanel -> panel child -> frame child identity. This only reads
    // the real parent ScrollArea; it neither stores state nor requests focus or scrolling.
    let scroll_id = egui::Id::new((h.ctx.viewport_id(), "central_panel"))
        .with(egui::Id::new("child"))
        .with(egui::Id::new("child"))
        .with(egui::Id::new((
            "localization-workbench",
            h.app.localization_ui.advanced,
            h.app.localization_ui.workbench.detail,
        )));
    let scroll = egui::scroll_area::State::load(&h.ctx, scroll_id)
        .expect("diagnostics must read the actual localization parent ScrollArea");
    let focus = h.ctx.memory(|memory| memory.focused());
    let widget = focus.and_then(|id| {
        h.ctx
            .viewport(|viewport| viewport.prev_pass.widgets.get(id).copied())
    });
    format!(
        "case={:?} actual frame={} events={events} focus={focus:?} widget={widget:?} scroll_id={scroll_id:?} scroll={scroll:?} pending={} notice={:?} accepted_export={} input={:?}",
        std::thread::current().name(), h.ctx.cumulative_frame_nr(), h.app.localization_ui.jobs.pending(),
        h.app.localization_ui.jobs.notice, h.app.localization_ui.export_plan.is_some(),
        h.ctx.input(|input| (input.time, input.smooth_scroll_delta, input.pointer.velocity()))
    )
}

#[test]
fn localization_short_keyboard_later_pointer_scroll_is_not_pulled_to_old_focus() {
    let (mut h, field) = typed_detail();
    let baseline = h.app.project.content_baseline();
    let history = h.app.history.len();
    let add = tab(&mut h, "添加文字段", false);
    let original = current_widget(&h, add).rect;
    tab_field(&mut h, field, "translation before new key", true);
    h.frame(vec![Event::Key {
        key: Key::Tab,
        physical_key: Some(Key::Tab),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    let point = egui::pos2(h.ctx.screen_rect().center().x, 180.0);
    h.frame(vec![
        Event::PointerMoved(point),
        Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 10_000.0),
            modifiers: Modifiers::NONE,
        },
    ]);
    for _ in 0..90 {
        h.frame(vec![]);
    }
    assert_eq!(focused(&h), add);
    let after = current_widget(&h, add).rect;
    assert!(
        after.top() > original.top() + 40.0,
        "pointer scroll must really move away from the old focus"
    );
    for _ in 0..6 {
        h.frame(vec![]);
    }
    assert!(
        (current_widget(&h, add).rect.top() - after.top()).abs() < 0.5,
        "stale keyboard focus must not pull the viewport back"
    );
    assert_typed_unchanged(&h, &baseline, history);
}

fn click_control(h: &mut Harness, label: &str, coalesced: bool, settle_open: bool) -> egui::Id {
    let label_point = h.visible(label);
    let id = h
        .ctx
        .viewport(|viewport| {
            viewport
                .prev_pass
                .widgets
                .layers()
                .flat_map(|(_, widgets)| widgets.iter())
                .filter(|widget| {
                    widget.enabled
                        && widget.sense.senses_click()
                        && widget.interact_rect.contains(label_point)
                })
                .min_by(|a, b| a.rect.area().total_cmp(&b.rect.area()))
                .map(|widget| widget.id)
        })
        .expect("the visible label belongs to a real clickable control");
    let mut origin = None;
    for _ in 0..100 {
        h.frame(vec![]);
        let widget = h
            .ctx
            .viewport(|viewport| viewport.prev_pass.widgets.get(id).copied())
            .expect("the same actual control stays registered");
        if h.ctx.screen_rect().contains_rect(widget.rect)
            && widget.interact_rect.contains_rect(widget.rect)
        {
            origin = Some(widget.rect);
            break;
        }
        assert!(
            widget.rect.left() >= h.ctx.screen_rect().left()
                && widget.rect.right() <= h.ctx.screen_rect().right(),
            "horizontal overflow cannot be repaired by vertical mouse setup: {:?}",
            widget.rect
        );
        let delta = if widget.rect.top() < widget.interact_rect.top() {
            widget.interact_rect.top() - widget.rect.top()
        } else {
            widget.interact_rect.bottom() - widget.rect.bottom()
        };
        h.wheel(h.scroll_point(), delta.clamp(-38.0, 38.0));
    }
    let before = origin.expect("the complete real control must be visible before clicking");
    let point = before.center();
    // Both native delivery schedules are legal: separate movement, or move+press in one frame.
    if !coalesced {
        h.frame(vec![Event::PointerMoved(point)]);
        h.frame(vec![]);
    }
    for pressed in [true, false] {
        let mut events = Vec::new();
        if pressed && coalesced {
            events.push(Event::PointerMoved(point));
        }
        events.push(Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
        h.frame(events);
    }
    let clicked = h
        .ctx
        .interaction_snapshot(|snapshot| snapshot.clicked)
        .expect("pointer release must click the real control");
    assert_eq!(clicked, id);
    let released = current_widget(h, clicked).rect;
    if settle_open {
        h.settle();
        let mut last = None;
        let mut stable = 0;
        for frame in 0..12 {
            h.frame(vec![]);
            let widget = current_widget(h, clicked);
            eprintln!(
                "actual open frame {frame}: rect={:?}, clipped={:?}",
                widget.rect, widget.interact_rect
            );
            if h.ctx.screen_rect().contains_rect(widget.rect)
                && widget.interact_rect.contains_rect(widget.rect)
                && last.is_some_and(|old: egui::Rect| {
                    old.min.distance(widget.rect.min) < 0.25
                        && old.max.distance(widget.rect.max) < 0.25
                })
            {
                stable += 1;
            } else {
                stable = 0;
            }
            last = Some(widget.rect);
            if stable == 2 {
                break;
            }
        }
        assert_eq!(
            stable, 2,
            "open anchor must settle wholly visible without pointer help"
        );
        let origin = current_widget(h, clicked).rect;
        for _ in 0..6 {
            h.frame(vec![]);
            let widget = current_widget(h, clicked);
            assert!(
                h.ctx.screen_rect().contains_rect(widget.rect)
                    && widget.interact_rect.contains_rect(widget.rect)
                    && widget.rect.min.distance(origin.min) < 0.5,
                "opened anchor must remain wholly visible after stable layout: {:?}",
                widget
            );
        }
    }
    eprintln!("complete click_control {label:?} coalesced={coalesced}: before={before:?}; release={released:?}; settled={:?}; target={}",
        (current_widget(h, clicked).rect, current_widget(h, clicked).interact_rect),
        h.app.localization_ui.target_locale);
    clicked
}

fn registered_locale_combo(coalesced: bool, scroll_after_open: bool) {
    let mut h = Harness::new(SOURCE);
    let mut selection = h.app.localization_ui.selection();
    selection.string_ids = vec!["line0".into()];
    let mut exchange = h
        .app
        .project
        .preview_localization_export(&selection)
        .unwrap()
        .exchange;
    exchange.entries[0].translation_parts = Some(vec![LocalizationPart::Text {
        text: "已保存譯文".into(),
    }]);
    let plan = h
        .app
        .project
        .preview_localization_import_candidate(&selection, &exchange)
        .unwrap();
    h.app
        .project
        .apply_localization_import_candidate(&selection, &exchange, &plan.plan_digest)
        .unwrap();
    h.app.localization_ui.workbench.invalidate();
    h.settle();
    assert_eq!(
        h.app
            .localization_ui
            .workbench
            .page
            .as_ref()
            .unwrap()
            .available_locales,
        ["zh-Hant"]
    );
    let baseline = h.app.project.content_baseline();
    h.replace("zh-Hant", "fr");
    let combo = click_control(&mut h, "已有语言", coalesced, !scroll_after_open);
    assert!(
        egui::ComboBox::is_open(&h.ctx, combo),
        "the real locale dropdown must open"
    );
    if scroll_after_open {
        let before = current_widget(&h, combo).rect;
        h.wheel(h.scroll_point(), 10_000.0);
        let after = current_widget(&h, combo).rect;
        assert!(after.top() > before.top() + 40.0,
            "real pointer scrolling must move away from the just-opened anchor: {before:?} -> {after:?}");
        for _ in 0..6 {
            h.frame(vec![]);
        }
        assert!(
            (current_widget(&h, combo).rect.top() - after.top()).abs() < 0.5,
            "opening reveal must not pull back a newer pointer scroll"
        );
        assert_eq!(h.app.localization_ui.target_locale, "fr");
        assert_eq!(h.app.project.content_baseline(), baseline);
        assert!(!h.app.localization_ui.has_unsubmitted_work());
        return;
    }
    let response = current_widget(&h, combo);
    assert!(h.ctx.screen_rect().contains_rect(response.rect)
        && response.interact_rect.contains_rect(response.rect),
        "registered locale dropdown including arrow must be wholly visible: rect={:?}, interact={:?}, screen={:?}",
        response.rect, response.interact_rect, h.ctx.screen_rect());
    h.click("zh-Hant");
    assert_eq!(h.app.localization_ui.target_locale, "zh-Hant");
    assert_eq!(
        h.app
            .localization_ui
            .workbench
            .page
            .as_ref()
            .unwrap()
            .target_locale
            .as_deref(),
        Some("zh-Hant")
    );
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(!h.app.localization_ui.has_unsubmitted_work());
}

#[test]
fn localization_short_registered_locale_combo_is_wholly_visible_and_selectable() {
    registered_locale_combo(false, false);
}

#[test]
fn localization_short_registered_locale_coalesced_pointer_keeps_complete_open_combo() {
    registered_locale_combo(true, false);
}

#[test]
fn localization_short_registered_locale_opening_reveal_respects_new_pointer_scroll() {
    registered_locale_combo(true, true);
}
