use super::*;

fn long_source() -> String {
    (0..60)
        .map(|i| format!("// 第{i}行 {}\n", "长行中文🌊正文，".repeat(100)))
        .collect()
}
fn source_geometry(output: &egui::FullOutput, source: &str) -> (egui::Pos2, Rect) {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == source => {
                Some((text.pos, shape.clip_rect))
            }
            _ => None,
        })
        .expect("唯一真实源码视口仍需绘制")
}

fn preview_target(output: &egui::FullOutput, preview: &SourceJumpPreview) -> (Rect, Rect) {
    let caption = super::super::view::preview_text(preview);
    let marker = preview.position.column - preview.context.start_column
        + usize::from(preview.context.truncated_start);
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == caption => Some((
                text.galley
                    .pos_from_cursor(CCursor::new(marker))
                    .translate(text.pos.to_vec2()),
                shape.clip_rect,
            )),
            _ => None,
        })
        .expect("有界目标上下文仍需绘制")
}

#[test]
fn source_jump_fixed_heading_never_moves_source_for_caret_or_error_state() {
    let mut h = Harness::new(source());
    h.select(1, 1);
    let before = source_geometry(&h.frame(vec![]), source());
    let index = SourceCoordinates::new(source()).unwrap();
    for request in ["7:8", "14:1", "1:1", "11:4"] {
        let target = index.locate(source(), request).unwrap();
        h.select(target.character_offset, target.character_offset);
        assert_eq!(source_geometry(&h.frame(vec![]), source()), before);
    }
    h.app.ime_composing = true;
    assert_eq!(source_geometry(&h.frame(vec![]), source()), before);
}

#[test]
fn source_jump_compact_window_long_line_preview_themes_sizes_and_cancel_geometry() {
    let source = long_source();
    let mut h = Harness::new(&source);
    for theme in [
        crate::theme::ThemeMode::Dark,
        crate::theme::ThemeMode::Light,
    ] {
        for (size, font) in [
            (egui::vec2(1040.0, 660.0), 16.0),
            (egui::vec2(1040.0, 660.0), 28.0),
            (egui::vec2(800.0, 600.0), 16.0),
            (egui::vec2(800.0, 600.0), 28.0),
        ] {
            h.app.personal.settings.theme = theme;
            h.app.personal.settings.body_size = font;
            h.size = size;
            h.select(1, 1);
            let before = source_geometry(&h.settle(), &source);
            h.open();
            h.query("40:400");
            let output = h.settle();
            let screen = Rect::from_min_size(egui::Pos2::ZERO, size);
            for label in [
                "跳转到行列",
                "定位并收起",
                "取消",
                "目标：第 40 行 · 第 400 列",
            ] {
                let rect = text_rect(&output, label)
                    .unwrap_or_else(|| panic!("missing {label}: {}", all_text(&output)));
                assert!(
                    screen.contains_rect(rect),
                    "{label} {rect:?} {size:?} font{font}"
                );
            }
            let preview = h
                .app
                .source_jump
                .review
                .as_ref()
                .unwrap()
                .preview
                .as_ref()
                .unwrap();
            assert!(preview.context.truncated_start && preview.context.truncated_end);
            assert!(preview.context.text.chars().count() <= 240);
            assert!(all_text(&output).contains("长行局部预览"));
            let (target, clip) = preview_target(&output, preview);
            assert!(
                clip.contains_rect(target),
                "首次预览目标需可见：{target:?}/{clip:?}, {size:?}/{font}"
            );
            if font == 28.0 {
                let preview = preview.clone();
                h.frame(vec![
                    Event::PointerMoved(clip.center()),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, 100_000.0),
                        modifiers: Modifiers::NONE,
                    },
                ]);
                let manual = h.settle();
                let (target, clip) = preview_target(&manual, &preview);
                assert!(!clip.contains(target.center()), "手动滚动不能被抢回目标");
                h.query("40:650");
                let updated = h.settle();
                let preview = h
                    .app
                    .source_jump
                    .review
                    .as_ref()
                    .unwrap()
                    .preview
                    .as_ref()
                    .unwrap();
                let (target, clip) = preview_target(&updated, preview);
                assert!(
                    clip.contains_rect(target),
                    "新查询应重新显示目标：{target:?}/{clip:?}"
                );
            }
            h.press(Key::Escape, Modifiers::NONE);
            assert_eq!(source_geometry(&h.settle(), &source), before);
            assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.source_id()));
        }
    }
}

#[test]
fn source_jump_back_restores_reversed_selection_scroll_and_wrap_layout() {
    let source = long_source();
    let mut h = Harness::new(&source);
    for wrap in [false, true] {
        h.app.personal.settings.source_wrap = wrap;
        h.select(14, 5);
        h.app.personal.source_scroll = [if wrap { 0.0 } else { 210.0 }, 780.0];
        h.app.personal.restore_source = true;
        h.settle();
        let scroll = h.app.personal.source_scroll;
        let selected = h.range();
        h.jump("59:400");
        assert_eq!(h.range().primary, h.range().secondary);
        h.press(Key::ArrowLeft, Modifiers::ALT);
        h.settle();
        assert_eq!(h.range(), selected);
        for (expected, actual) in scroll.into_iter().zip(h.app.personal.source_scroll) {
            assert!(
                (expected - actual).abs() < 1.5,
                "{expected} / {actual}, wrap={wrap}"
            );
        }
        assert_eq!(h.source(), source);
    }
}

#[test]
fn source_jump_filename_stays_left_aligned_with_fixed_heading_and_position_budget() {
    let mut h = Harness::new(source());
    let coordinates = SourceCoordinates::new(source()).unwrap();
    let inside_crlf = source()[..source().find('\r').unwrap()].chars().count() + 1;
    let long_name = format!("{}.wl", "潮汐档案馆第十三灯室记录".repeat(10));
    for mode in [
        crate::theme::ThemeMode::Dark,
        crate::theme::ThemeMode::Light,
    ] {
        for size in [egui::vec2(1040.0, 660.0), egui::vec2(800.0, 600.0)] {
            for font in [16.0, 28.0] {
                let mut baseline = None;
                for name in ["world.wl", long_name.as_str()] {
                    for character in [
                        coordinates
                            .locate(source(), "1:1")
                            .unwrap()
                            .character_offset,
                        coordinates
                            .locate(source(), "11:9")
                            .unwrap()
                            .character_offset,
                        inside_crlf,
                    ] {
                        h.select(character, character);
                        crate::theme::configure(&h.ctx, mode);
                        h.ctx.style_mut(|style| {
                            style
                                .text_styles
                                .insert(egui::TextStyle::Body, egui::FontId::proportional(font));
                            style
                                .text_styles
                                .insert(egui::TextStyle::Button, egui::FontId::proportional(font));
                            style.text_styles.insert(
                                egui::TextStyle::Heading,
                                egui::FontId::proportional(font * 1.25),
                            );
                        });
                        let mut geometry = None;
                        h.tick += 1;
                        let output = h.ctx.run(
                            egui::RawInput {
                                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                                time: Some(f64::from(h.tick) / 60.0),
                                ..Default::default()
                            },
                            |ctx| {
                                egui::CentralPanel::default().show(ctx, |ui| {
                                    let origin = ui.cursor().min;
                                    let response = h.app.source_jump_heading(ctx, ui, name);
                                    geometry = Some((origin, response.rect, ui.min_rect()));
                                });
                            },
                        );
                        let (origin, position, whole) = geometry.unwrap();
                        let title = text_rect(&output, name).expect("文件名应在固定预算内绘制");
                        assert!(
                            (title.left() - origin.x).abs() < 0.1,
                            "长短文件名均须贴同一左边界：{name:?} {title:?}/{origin:?}"
                        );
                        assert!(title.right() <= position.left(), "长标题不能侵入行列入口");
                        let actual = (title.left(), position, whole);
                        if let Some(expected) = baseline {
                            assert_eq!(
                                actual, expected,
                                "标题长短、行列位数和错误提示不能移动固定布局"
                            );
                        } else {
                            baseline = Some(actual);
                        }
                        assert_eq!(h.source(), source());
                    }
                }
            }
        }
    }
}
