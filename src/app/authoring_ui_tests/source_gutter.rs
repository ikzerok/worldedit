use super::*;

fn galley_for(
    shape: &egui::Shape,
    text: &str,
) -> Option<(egui::Pos2, std::sync::Arc<egui::Galley>)> {
    match shape {
        egui::Shape::Text(shape) if shape.galley.job.text == text => {
            Some((shape.pos, shape.galley.clone()))
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| galley_for(shape, text)),
        _ => None,
    }
}

fn gutter_y(output: &egui::FullOutput, line: usize, old_numbers: &str) -> Option<f32> {
    for shape in &output.shapes {
        if let Some((pos, galley)) = galley_for(&shape.shape, &line.to_string()) {
            return Some(pos.y + galley.rect.center().y);
        }
        if let Some((pos, galley)) = galley_for(&shape.shape, old_numbers) {
            let offset = old_numbers
                .split_inclusive('\n')
                .take(line - 1)
                .map(|s| s.chars().count())
                .sum();
            return Some(
                pos.y
                    + galley
                        .pos_from_cursor(egui::text::CCursor::new(offset))
                        .center()
                        .y,
            );
        }
    }
    None
}

#[test]
fn long_source_jump_gutter_tracks_physical_lines_and_trailing_empty_line() {
    for style_size in [14.0, 22.0] {
        let (ctx, mut app) = app();
        ctx.style_mut(|style| {
            style.scroll_animation = egui::style::ScrollAnimation::none();
            style.text_styles.insert(
                egui::TextStyle::Monospace,
                egui::FontId::monospace(style_size),
            );
        });
        let lines = (1..=2100)
            .map(|line| {
                if line == 2033 {
                    "// TARGET_2033 中文".into()
                } else if line % 7 == 1 || line % 7 == 2 {
                    String::new()
                } else {
                    format!("// mixed 中文 ASCII {line}")
                }
            })
            .collect::<Vec<String>>();
        let source = lines.join("\n") + "\n";
        let old_numbers = (1..=source.lines().count())
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let path = app.active_file.clone();
        app.project.set_text(&path, source.clone()).unwrap();
        app.recompile();
        let baseline = app.project.content_baseline();
        for _ in 0..8 {
            let _ = frame(&ctx, &mut app, Vec::new(), 11);
        }
        for line in [2033usize, 2101] {
            app.jump = Some((line as u32, 1));
            for _ in 0..8 {
                let _ = frame(&ctx, &mut app, Vec::new(), 11);
            }
            let output = frame(&ctx, &mut app, Vec::new(), 11);
            let (position, galley) = output
                .shapes
                .iter()
                .find_map(|shape| galley_for(&shape.shape, &source))
                .expect("源码必须仍然显示");
            let offset = source
                .split_inclusive('\n')
                .take(line - 1)
                .map(|s| s.chars().count())
                .sum();
            let cursor = egui::text::CCursor::new(offset);
            let source_y = position.y + galley.pos_from_cursor(cursor).center().y;
            let number_y =
                gutter_y(&output, line, &old_numbers).unwrap_or_else(|| panic!("行{line}字号{style_size}源码位置{source_y}，可见物理行必须有行号，包括末尾空行"));
            assert!(
                (source_y - number_y).abs() <= 1.0,
                "字号{style_size} 行{line}累计漂移：正文{source_y}，行号{number_y}"
            );
            let state = egui::TextEdit::load_state(&ctx, egui::Id::new(("source", &path))).unwrap();
            assert_eq!(state.cursor.char_range().unwrap().primary.index, offset);
            assert_eq!(app.project.content_baseline(), baseline);
        }
    }
}

#[test]
fn highlighted_layout_keeps_every_source_character_once_including_whitespace() {
    for source in [
        "",
        "\n",
        "\n\n",
        "  entity a\r\n\r\n\t中文~\r\n",
        "正文~  ",
        "#tag~",
        "#tag~  ",
        "{中文~  ",
        "  ->   ",
        "  entity a kind place as \"中文\"\n\n  正文~  \n  -> END\n",
        "/*\n\n中文\n*/\n",
    ] {
        for size in [11.0, 14.0, 22.0] {
            let job =
                crate::highlight::layout_job(source, size, worldline_core::LanguageVersion::V1_10);
            assert_eq!(job.text, source);
            let mut covered = 0;
            for section in &job.sections {
                assert_eq!(section.byte_range.start, covered);
                assert!(section.byte_range.end >= covered);
                assert_eq!(section.format.font_id, egui::FontId::monospace(size));
                assert_ne!(section.format.color.a(), 0);
                covered = section.byte_range.end;
            }
            assert_eq!(covered, source.len());
            let rendered: String = job
                .sections
                .iter()
                .map(|section| &job.text[section.byte_range.clone()])
                .collect();
            assert_eq!(
                rendered, source,
                "高亮不能遗漏或重复字符，否则光标与选择偏移不再对应正文"
            );
        }
    }
}
