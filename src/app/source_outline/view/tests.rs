//! 与先前居中按钮逐项比对外框/换行；只改变文字在全宽行内的水平对齐。
use super::*;
use std::sync::Arc;

#[test]
fn source_outline_rows_left_align_name_and_metadata_without_changing_size_or_wrap() {
    let ctx = egui::Context::default();
    crate::fonts::install_cjk_fonts(&ctx);
    for mode in [theme::ThemeMode::Dark, theme::ThemeMode::Light] {
        let _theme = theme::configure(&ctx, mode);
        for size in [16.0, 28.0] {
            for width in [300.0, 620.0] {
                for (depth, display) in [
                    (0, "灯室".to_owned()),
                    (1, "同名灯室的分支".to_owned()),
                    (
                        2,
                        "潮汐档案馆第十三灯室记忆借阅登记与临时保密事项".repeat(3),
                    ),
                ] {
                    let entry = SourceOutlineEntry {
                        occurrence: 0,
                        parent: None,
                        depth,
                        kind: "scene".into(),
                        id: "start.gate.inner".into(),
                        display,
                        entity_type: None,
                        line: 12,
                        header: 0..10,
                        body: 0..20,
                    };
                    let mut boxes = None;
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(900.0, 2200.0),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            egui::CentralPanel::default().show(ctx, |ui| {
                                ui.allocate_ui_with_layout(
                                    egui::vec2(width, 0.0),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        let aligned = row(ui, &entry, true, false, size);
                                        let legacy = ui
                                            .horizontal(|ui| {
                                                ui.add_space(entry.depth as f32 * 12.0);
                                                ui.add_sized(
                                                    [ui.available_width(), 0.0],
                                                    egui::Button::selectable(
                                                        true,
                                                        row_text(ui, &entry, true, size),
                                                    )
                                                    .wrap(),
                                                )
                                            })
                                            .inner;
                                        boxes = Some((
                                            aligned.rect,
                                            legacy.rect,
                                            ui.spacing().button_padding.x,
                                        ));
                                    },
                                );
                            });
                        },
                    );
                    let (aligned, legacy, padding) = boxes.unwrap();
                    assert!(aligned.width() <= width, "测试必须在实际指定宽度内绘制");
                    assert_eq!(
                        aligned.size(),
                        legacy.size(),
                        "行外框与高度须保持：{mode:?}, {size}, {width}, {depth}"
                    );
                    let mut text = Vec::new();
                    for shape in &output.shapes {
                        collect(&shape.shape, &entry.display, &mut text);
                    }
                    assert_eq!(text.len(), 2);
                    let (position, galley) = &text[0];
                    let (old_position, old_galley) = &text[1];
                    assert_eq!(
                        galley.rows.len(),
                        old_galley.rows.len(),
                        "换行数必须与旧布局相同"
                    );
                    for (line, old_line) in galley.rows.iter().zip(&old_galley.rows) {
                        assert_eq!(
                            line.glyphs.len(),
                            old_line.glyphs.len(),
                            "每行字符不可因对齐改变"
                        );
                        assert!(line.pos.x.abs() < 0.1, "名称和metadata均须左对齐");
                    }
                    assert!((position.x - aligned.left() - padding).abs() < 1.0,
                        "文字应贴统一左内边距，不随标题长度居中：{position:?}, {aligned:?}, padding={padding}");
                    if galley.rect.width() < aligned.width() - 30.0 {
                        assert!(
                            old_position.x > position.x + 5.0,
                            "短标签须实际从居中移到左边"
                        );
                    }
                    assert!(
                        position.x + galley.rect.right() <= aligned.right() + 0.5,
                        "右侧也须在原行边界内"
                    );
                }
            }
        }
    }
}

fn collect(shape: &egui::Shape, name: &str, out: &mut Vec<(egui::Pos2, Arc<egui::Galley>)>) {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text.starts_with(name) => {
            out.push((text.pos, text.galley.clone()))
        }
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                collect(shape, name, out);
            }
        }
        _ => {}
    }
}
