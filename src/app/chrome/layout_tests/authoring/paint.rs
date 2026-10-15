use super::*;

fn contrast(a: Color32, b: Color32) -> f64 {
    fn luminance(color: Color32) -> f64 {
        let linear = |channel: u8| {
            let c = f64::from(channel) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
fn over(front: Color32, back: Color32) -> Color32 {
    let [r, g, b, a] = front.to_srgba_unmultiplied();
    let alpha = f64::from(a) / 255.0;
    let blend = |x: u8, y: u8| (f64::from(x) * alpha + f64::from(y) * (1.0 - alpha)).round() as u8;
    Color32::from_rgb(blend(r, back.r()), blend(g, back.g()), blend(b, back.b()))
}

/// 对实际图元顺序、opacity 和所在表面计算，不把调色板 token 当渲染结果。
pub(super) fn assert_state_contrast(output: &egui::FullOutput, label: &str, target: Rect) {
    fn gather<'a>(shape: &'a egui::Shape, clip: Rect, shapes: &mut Vec<(&'a egui::Shape, Rect)>) {
        match shape {
            egui::Shape::Vec(children) => {
                for child in children {
                    gather(child, clip, shapes);
                }
            }
            _ => shapes.push((shape, clip)),
        }
    }
    let mut shapes = Vec::new();
    for shape in &output.shapes {
        gather(&shape.shape, shape.clip_rect, &mut shapes);
    }
    let mut found = false;
    for (index, (shape, clip)) in shapes.iter().enumerate() {
        let egui::Shape::Text(text) = shape else {
            continue;
        };
        if text.galley.text() != label {
            continue;
        }
        let rect = text.galley.rect.translate(text.pos.to_vec2());
        if !target.contains(rect.center()) {
            continue;
        }
        found = true;
        assert!(
            clip.contains_rect(rect),
            "full state text, not only its center, must be visible"
        );
        let mut background = Color32::TRANSPARENT;
        let mut opaque = false;
        for (previous, previous_clip) in &shapes[..index] {
            if let egui::Shape::Rect(painted) = previous {
                if painted.rect.contains(rect.center()) && previous_clip.contains(rect.center()) {
                    opaque |= painted.fill.a() == 255;
                    background = over(painted.fill, background);
                }
            }
        }
        assert!(opaque);
        for section in &text.galley.job.sections {
            assert!(
                section.format.font_id.size >= 14.0,
                "state text keeps the UI font size"
            );
            let color = text
                .override_text_color
                .unwrap_or_else(|| {
                    if section.format.color == Color32::PLACEHOLDER {
                        text.fallback_color
                    } else {
                        section.format.color
                    }
                })
                .gamma_multiply(text.opacity_factor);
            assert!(
                contrast(over(color, background), background) >= 4.5,
                "{label}: {color:?}/{background:?}"
            );
        }
    }
    assert!(found, "missing state label {label}");
}
