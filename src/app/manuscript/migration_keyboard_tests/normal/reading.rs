use super::*;

pub(super) fn read_exact(h: &mut Harness, pages: &[String]) {
    let region = h.tab_to(NORMAL_READ).id;
    for _ in 0..2000 {
        if h.offset() < 0.1 {
            break;
        }
        h.key(Key::ArrowUp, Modifiers::NONE);
    }
    assert!(h.offset() < 0.1, "reading can return to document top");
    let mut expected = std::collections::BTreeSet::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut found = std::collections::BTreeSet::new();
    let mut stopped = 0;
    for _ in 0..2000 {
        let old = h.offset();
        let out = h.key(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(h.ctx.memory(|m| m.focused()), Some(region));
        assert!(h.reading_focus_visible(&out));
        for (full, index, row, visible) in galley_rows(&out) {
            for (side, text) in pages
                .iter()
                .enumerate()
                .filter(|(_, text)| !text.trim().is_empty())
            {
                if &full == text {
                    found.insert(side);
                    expected.insert((side, index, row.clone()));
                    if visible {
                        seen.insert((side, index, row.clone()));
                    }
                }
            }
        }
        stopped = if (h.offset() - old).abs() < 0.1 {
            stopped + 1
        } else {
            0
        };
        if stopped == 3 {
            break;
        }
    }
    assert_eq!(
        found,
        pages
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.trim().is_empty())
            .map(|(i, _)| i)
            .collect(),
        "all exact core strings must be painted"
    );
    assert_eq!(
        seen, expected,
        "every nonempty wrapped row must enter the actual clip"
    );
    assert_eq!(stopped, 3, "reading reaches bottom");
    let bottom = h.offset();
    h.key(Key::ArrowDown, Modifiers::NONE);
    assert!((h.offset() - bottom).abs() < 0.1);
}
fn page(text: &str, n: usize) -> String {
    let boundary = |mut at: usize| {
        at = at.min(text.len());
        while !text.is_char_boundary(at) {
            at -= 1;
        }
        at
    };
    text[boundary(n * 16 * 1024)..boundary((n + 1) * 16 * 1024)].to_owned()
}
#[test]
fn normal_preview_keyboard_long_utf8_pages_are_readable_in_short_scaled_viewport() {
    let mut h = Harness::normal("");
    h.begin_fields();
    let long = (0..240)
        .map(|n| format!("第{n:03}行 ordinary preview：完整核对正文，不截断多字节字符。\n"))
        .collect::<String>();
    replace(&mut h, BODY, &long);
    // Existing documented next-line shortcut explicitly generates this plan;
    // no offscreen Preview click or direct plan/state injection is needed.
    h.key(Key::Enter, Modifiers::COMMAND);
    let plan = normal_plan(&h);
    assert!(plan.before.is_empty());
    assert!(plan.after.len() > 16 * 1024 && plan.after.len() < 32 * 1024);
    assert!(plan.can_apply);
    h.size = egui::vec2(800.0, 600.0);
    h.app.personal.settings.appearance.ui_scale = 1.25;
    h.settle();
    let state = h.state();
    let fields = h.fields();
    read_exact(&mut h, &[page(&plan.after, 0)]);
    let first = format!("第1 / 2段 · {} UTF-8字节", plan.after.len());
    let second = format!("第2 / 2段 · {} UTF-8字节", plan.after.len());
    h.tab_near("后一段字节", Some(&first));
    h.key(Key::Enter, Modifiers::NONE);
    read_exact(&mut h, &[page(&plan.after, 1)]);
    h.tab_near("前一段字节", Some(&second));
    h.key(Key::Enter, Modifiers::NONE);
    read_exact(&mut h, &[page(&plan.after, 0)]);
    h.tab_to("纳入并写下一句");
    assert_eq!(h.fields(), fields);
    assert_eq!(h.state(), state);
    assert_eq!(h.app.project.language_version(), "1.11");
}
