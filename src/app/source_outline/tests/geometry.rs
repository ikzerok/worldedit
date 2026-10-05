use super::*;

fn collection() -> String {
    (0..60).map(|i| format!("event part_{i:02} as \"潮汐档案馆第十三灯室记忆借阅登记与临时保密事项第{i:02}份记录\"\n  {}\n  -> END\n", "长行中文🌊正文，".repeat(100))).collect()
}

#[test]
fn source_outline_narrow_large_type_two_themes_wrap_and_close_without_squeezing_source() {
    let mut h = Harness::new(&collection());
    for theme in [
        crate::theme::ThemeMode::Dark,
        crate::theme::ThemeMode::Light,
    ] {
        for (size, font) in [
            (egui::vec2(800.0, 600.0), 16.0),
            (egui::vec2(800.0, 600.0), 28.0),
            (egui::vec2(1188.0, 848.0), 28.0),
        ] {
            h.app.personal.settings.theme = theme;
            h.app.personal.settings.body_size = font;
            h.size = size;
            h.select(3, 3);
            let before = h.settle();
            let source_before = text_rect(&before, h.source()).unwrap();
            h.open();
            h.press(Key::End, Modifiers::NONE);
            let output = h.settle();
            assert_eq!(h.app.source_outline.selected, 59);
            let entry = &h.app.source_outline.cache.as_ref().unwrap().outline.entries[59];
            let label = format!("{}\n{}", entry.display, metadata(entry));
            let rect = text_rect(&output, &label)
                .unwrap_or_else(|| panic!("末项未显示：{theme:?} {size:?} {font}"));
            let screen = Rect::from_min_size(egui::Pos2::ZERO, size);
            assert!(
                screen.contains_rect(rect),
                "长中文与完整ID须在小窗换行可达：{rect:?} / {screen:?}"
            );
            assert!(rect.height() >= font * 2.0);
            let close = text_rect(&output, "收起并回到源码").unwrap();
            assert!(screen.contains_rect(close));
            assert!(all_text(&output).contains("当前位置"));
            h.press(Key::Escape, Modifiers::NONE);
            let after = h.settle();
            let source_after = text_rect(&after, h.source()).unwrap();
            assert_eq!(source_before, source_after, "浮层收起不改变源码面积和位置");
            assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.source_id()));
        }
    }
}

#[test]
fn source_outline_full_app_back_restores_long_line_scroll_and_selection_direction() {
    let source = collection();
    let mut h = Harness::new(&source);
    let at = source[..source.find("part_12").unwrap()].chars().count();
    h.select(at + 5, at + 1);
    h.app.personal.source_scroll = [220.0, 780.0];
    h.app.personal.restore_source = true;
    h.settle();
    let before_scroll = h.app.personal.source_scroll;
    let before_range = h.range();
    assert!(
        before_scroll[0] > 0.0 && before_scroll[1] > 0.0,
        "真实双向长行滚动：{before_scroll:?}"
    );
    h.open();
    h.press(Key::End, Modifiers::NONE);
    h.settle();
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(h.app.personal.source_scroll[1] > before_scroll[1]);
    h.press(Key::ArrowLeft, Modifiers::ALT);
    h.settle();
    assert_eq!(h.range(), before_range);
    for (before, after) in before_scroll.into_iter().zip(h.app.personal.source_scroll) {
        assert!(
            (before - after).abs() < 1.5,
            "应还原真实滚动：{before} / {after}"
        );
    }
    assert_eq!(h.source(), source);
}

#[test]
fn source_outline_hidden_selection_does_not_jump_and_keyboard_reveals_it() {
    let mut h = Harness::new(&collection());
    h.open();
    h.press(Key::End, Modifiers::NONE);
    let output = h.settle();
    let entry = &h.app.source_outline.cache.as_ref().unwrap().outline.entries[59];
    let label = format!("{}\n{}", entry.display, metadata(entry));
    let point = text_rect(&output, &label).unwrap().center();
    h.frame(vec![
        Event::PointerMoved(point),
        Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 100_000.0),
            modifiers: Modifiers::NONE,
        },
    ]);
    let output = h.settle();
    assert!(text_rect(&output, &label).is_none());
    h.press(Key::Enter, Modifiers::NONE);
    assert!(h.app.source_outline.open);
    assert!(h.app.personal.history.is_empty());
    h.press(Key::ArrowUp, Modifiers::NONE);
    h.settle();
    h.press(Key::Enter, Modifiers::NONE);
    assert!(!h.app.source_outline.open);
}
