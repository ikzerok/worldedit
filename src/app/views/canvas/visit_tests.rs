//! egui 绘制指令、静态颜色合成与文字回归；不代表真实像素、读屏或物理高 DPI 验收。
use super::*;
use egui::{Color32, Context, RawInput, Shape};

fn node() -> GraphNode {
    GraphNode {
        name: "arrival".into(),
        is_event: true,
        file: "world.wl".into(),
        line: 3,
        choice_count: 0,
        word_count: 0,
        storyline: "main".into(),
        seq: 1,
        summary: Some("抵达港口".into()),
        characters: Vec::new(),
        perm: None,
    }
}

fn luminance(rgb: [f64; 3]) -> f64 {
    let linear = rgb.map(|value| {
        let value = value / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    });
    linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
}

fn rgb(color: Color32) -> [f64; 3] {
    [color.r(), color.g(), color.b()].map(f64::from)
}

fn contrast(foreground: Color32, background: [f64; 3]) -> f64 {
    let (a, b) = (luminance(rgb(foreground)), luminance(background));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

// Color32 存放预乘 sRGBA；按绘制顺序将卡片和阴影合成到不透明画布基色。
fn composite(foreground: Color32, background: [f64; 3]) -> [f64; 3] {
    let alpha = f64::from(foreground.a()) / 255.0;
    let front = rgb(foreground);
    std::array::from_fn(|i| front[i] + background[i] * (1.0 - alpha))
}

fn text_shapes(output: &egui::FullOutput) -> Vec<&egui::epaint::TextShape> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            Shape::Text(text) => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn visit_marker_contrast_tracks_actual_node_fills_in_light_and_dark_states() {
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        let ctx = Context::default();
        theme::configure(&ctx, mode);
        for selected in [false, true] {
            for hovered in [false, true] {
                for search in ["", "不匹配的搜索"] {
                    let rect = Rect::from_min_size(Pos2::new(30.0, 30.0), Vec2::new(WIDTH, HEIGHT));
                    let output = ctx.run(RawInput::default(), |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            draw_node(
                                ui.painter(),
                                rect,
                                &node(),
                                selected,
                                hovered,
                                NodeHeading::Sequence,
                                search,
                            );
                            draw_visit_marker(ui.painter(), rect, 1.0, 7);
                        });
                    });
                    let mut surface = rgb(BG());
                    let mut layers = 0;
                    for shape in &output.shapes {
                        if let Shape::Rect(fill) = &shape.shape {
                            if (fill.rect == rect
                                || fill.rect == rect.translate(Vec2::new(0.0, 4.0)))
                                && fill.fill.a() > 0
                            {
                                surface = composite(fill.fill, surface);
                                layers += 1;
                            }
                        }
                    }
                    assert_eq!(layers, 2, "应读取实际节点阴影和卡片填充");
                    let texts = text_shapes(&output);
                    let marker = texts
                        .iter()
                        .find(|text| text.galley.job.text == "访问 ×7")
                        .expect("访问次数必须保留明确文字，不能只靠颜色");
                    assert!(!marker.galley.job.sections.is_empty());
                    for section in &marker.galley.job.sections {
                        let color = section.format.color;
                        assert_eq!(color, SUCCESS());
                        assert_eq!(color.a(), 255, "搜索淡出不应淡化访问次数");
                        assert!(
                            contrast(color, surface) >= 4.5,
                            "{mode:?}, selected={selected}, hovered={hovered}, search={search:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn zoomed_visit_marker_has_exact_count_in_unscaled_hover_text() {
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        let ctx = Context::default();
        theme::configure(&ctx, mode);
        for zoom in [0.25, 0.5, 1.0, 1.6] {
            for count in [0, 1, 27, u32::MAX] {
                let output = ctx.run(RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let rect = Rect::from_min_size(
                            Pos2::new(30.0, 30.0),
                            Vec2::new(WIDTH, HEIGHT) * zoom,
                        );
                        draw_visit_marker(ui.painter(), rect, zoom, count);
                        node_hover_ui(ui, &node(), Some(count));
                    });
                });
                let label = format!("访问 ×{count}");
                let texts = text_shapes(&output);
                let labels: Vec<_> = texts
                    .iter()
                    .filter(|text| text.galley.job.text == label)
                    .collect();
                assert_eq!(labels.len(), 2, "标记与悬浮文字均保留完整次数");
                assert!(labels[1].galley.job.sections.iter().all(|section| section
                    .format
                    .font_id
                    .size
                    >= META_SIZE
                    && contrast(section.format.color, rgb(PANEL())) >= 4.5));
                assert!(texts
                    .iter()
                    .any(|text| text.galley.job.text.contains("world.wl:3")));
            }
        }
    }
}

#[test]
fn nodes_without_visit_counts_keep_no_coverage_label() {
    let ctx = Context::default();
    theme::configure(&ctx, ThemeMode::Light);
    let output = ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            node_hover_ui(ui, &node(), None);
        });
    });
    assert!(text_shapes(&output)
        .iter()
        .all(|text| !text.galley.job.text.contains("访问")));
}
