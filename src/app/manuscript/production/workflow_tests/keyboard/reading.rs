use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn production_keyboard_artifact_pages_and_readonly_scroll_are_complete() {
    let _serial = serial();
    let mut h = Keyboard::new(3, true, false);
    let disk = h.flow.disk();
    let baseline = h.flow.app.project.content_baseline();
    h.setup_generated();
    h.setup_preview("精确 JSON", false);
    let bytes = h.flow.artifact();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(
        text.len() > 16 * 1024,
        "real artifact must cross a UTF-8 byte page"
    );
    let boundary = |mut n: usize| {
        n = n.min(text.len());
        while !text.is_char_boundary(n) {
            n -= 1;
        }
        n
    };
    let count = text.len().div_ceil(16 * 1024);
    let author = h.author_state();
    let mut reconstructed = String::new();
    let mut vertical = false;
    for page in 0..count {
        let segment = &text[boundary(page * 16 * 1024)..boundary((page + 1) * 16 * 1024)];
        assert_eq!(h.flow.app.manuscript.production.artifact_page, page);
        // Real pointer setup positions the independent byte region; this is not
        // credited as keyboard traversal of the preceding list/format controls.
        h.pointer(if page == 0 {
            "预览完整交付字节"
        } else {
            "上一段交付字节"
        });
        if page > 0 {
            // The preceding setup button is a real action; return to this page
            // through the actual forward button before testing its byte region.
            h.pointer("下一段交付字节");
            assert_eq!(h.flow.app.manuscript.production.artifact_page, page);
        }
        let mut out = h.tab_to("交付字节阅读区 · ↑↓←→滚动");
        let owner = h
            .flow
            .ctx
            .memory(|m| m.focused())
            .expect("actual named reader owner");
        assert!(h
            .gained
            .iter()
            .any(|info| info.typ == egui::WidgetType::Button
                && info.label.as_deref() == Some("交付字节阅读区 · ↑↓←→滚动")));
        let parent = h.outer_clip.expect("actual parent paint clip");
        let mut area = inner_area(&h);
        if !parent.contains_rect(area.rect) || !area.interact_rect.contains_rect(area.rect) {
            assert_eq!(
                out.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
                Duration::ZERO,
                "nested domain reveal must request its next real paint"
            );
            out = h.frame(vec![]);
            assert_eq!(h.flow.ctx.memory(|m| m.focused()), Some(owner));
            area = inner_area(&h);
        }
        let domain = area.rect;
        assert!(domain.height() <= 300.5 && domain.height() > 0.0);
        assert!(parent.contains_rect(domain) && area.interact_rect.contains_rect(domain),
            "inner viewport must be wholly visible in its parent, independent of TextEdit padding; {}",h.diagnostic(&out));
        assert!(
            focus_paint(&h.flow.ctx, &out, domain, false),
            "named reader must paint its own actual inner viewport focus; {}",
            h.diagnostic(&out)
        );
        let (galley, clip) = texts(&out)
            .into_iter()
            .find_map(|(text, rect, clip)| (text == segment).then_some((rect, clip)))
            .expect("the exact core page is painted within the reader");
        let overflow_x = galley.width() > clip.width() + 0.5;
        let overflow_y = galley.height() > clip.height() + 0.5;
        let outer_before = outer_offset(&h);
        let mut page_horizontal = false;
        let mut page_vertical = false;
        let chars: Vec<_> = segment.chars().collect();
        let expected: BTreeSet<_> = chars
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(n, c)| (!c.is_whitespace()).then_some(n))
            .collect();
        for key in [Key::ArrowUp, Key::ArrowLeft] {
            for _ in 0..5000 {
                let offset = inner_offset(&h);
                if (key == Key::ArrowUp && offset.y <= 0.5)
                    || (key == Key::ArrowLeft && offset.x <= 0.5)
                {
                    break;
                }
                out = h.key(key, Modifiers::NONE);
                assert_ne!(
                    inner_offset(&h),
                    offset,
                    "real keyboard must return this byte region to its beginning"
                );
            }
        }
        assert_eq!(inner_offset(&h), egui::Vec2::ZERO);
        let mut seen = BTreeSet::new();
        let mut closest: BTreeMap<usize, (f32, Value)> = BTreeMap::new();
        let mut direction = Key::ArrowRight;
        for _ in 0..5000 {
            let (clip, characters) = glyphs_in(&h.flow.ctx, &out, segment, domain)
                .expect("same immutable UTF-8 segment");
            assert!(
                domain.contains_rect(clip),
                "glyph clip is local content, not the padded area frame"
            );
            let geometry = glyph_geometry(&h.flow.ctx, &out, segment);
            let before_meta = frame_meta(&h, &out, domain);
            for glyph in &geometry {
                if !seen.contains(&glyph.index) && !glyph.character.is_whitespace() {
                    let (fraction, sample) = glyph_sample(glyph, domain, &before_meta);
                    let prior = closest
                        .entry(glyph.index)
                        .or_insert((fraction, sample.clone()));
                    if fraction > prior.0 {
                        *prior = (fraction, sample);
                    }
                }
            }
            for (n, c, visible) in characters {
                if visible && !c.is_whitespace() {
                    assert_eq!(chars.get(n), Some(&c));
                    seen.insert(n);
                    closest.remove(&n);
                }
            }
            if seen == expected {
                break;
            }
            let before = inner_offset(&h);
            if overflow_x {
                out = h.key(direction, Modifiers::NONE);
                let after = inner_offset(&h);
                page_horizontal |= (after.x - before.x).abs() > 0.5;
                if (after.x - before.x).abs() < 0.5 {
                    out = h.key(Key::ArrowDown, Modifiers::NONE);
                    page_vertical |= (inner_offset(&h).y - after.y).abs() > 0.5;
                    direction = if direction == Key::ArrowRight {
                        Key::ArrowLeft
                    } else {
                        Key::ArrowRight
                    };
                }
            } else {
                out = h.key(Key::ArrowDown, Modifiers::NONE);
                page_vertical |= (inner_offset(&h).y - before.y).abs() > 0.5;
            }
            assert_eq!(
                h.flow.ctx.memory(|m| m.focused()),
                Some(owner),
                "arrows keep exact named byte owner"
            );
            assert_eq!(
                outer_offset(&h),
                outer_before,
                "inner arrows never move the outer workbench"
            );
            if inner_offset(&h) == before {
                let missing: BTreeSet<_> = expected.difference(&seen).copied().collect();
                let current_meta = frame_meta(&h, &out, domain);
                let current = glyph_geometry(&h.flow.ctx, &out, segment);
                let samples = |glyphs: &[GlyphGeometry], meta: &Value| {
                    glyphs
                        .iter()
                        .filter(|glyph| missing.contains(&glyph.index))
                        .map(|glyph| glyph_sample(glyph, domain, meta).1)
                        .collect::<Vec<_>>()
                };
                assert_ne!(inner_offset(&h), before,
                    "real overflow must advance while unread glyphs remain; seen={}/{} axes=({overflow_x},{overflow_y}); missing={missing:?}; closest={:#?}; previous={:#?}; current={:#?}; {}",
                    seen.len(), expected.len(), closest, samples(&geometry, &before_meta),
                    samples(&current, &current_meta), h.diagnostic(&out));
            }
        }
        assert_eq!(seen,expected,"every non-whitespace Unicode character becomes fully visible in the actual inner content clip");
        if overflow_x {
            assert!(
                page_horizontal,
                "a truly overflowing horizontal axis must move"
            );
        }
        if overflow_y {
            assert!(page_vertical, "a truly overflowing vertical axis must move");
        }
        vertical |= page_vertical;
        reconstructed.push_str(segment);
        if page + 1 < count {
            h.enter("下一段交付字节");
        }
    }
    assert!(
        vertical,
        "this real pretty JSON fixture must exercise vertical reading"
    );
    // Current default TextEdit wrapping may eliminate horizontal overflow. It is
    // required only for pages whose actual galley exceeds the real content clip.
    assert_eq!(reconstructed.as_bytes(), bytes);
    h.enter("上一段交付字节");
    assert_eq!(h.flow.app.manuscript.production.artifact_page, count - 2);
    h.enter(CONFIRM);
    assert!(h.flow.app.manuscript.production.confirmed);
    let out = h.enter("复制相同完整材料");
    assert_eq!(copied(&out).unwrap().as_bytes(), bytes);
    assert_eq!(h.flow.disk(), disk);
    assert_eq!(h.flow.app.project.content_baseline(), baseline);
    assert_eq!(h.author_state(), author);
}
fn inner_offset(h: &Keyboard) -> egui::Vec2 {
    h.offsets()
        .into_iter()
        .find(|(n, _, _)| n == "production-artifact-bytes")
        .expect("actual inner ScrollArea state")
        .2
}

fn inner_area(h: &Keyboard) -> egui::Response {
    h.areas
        .iter()
        .find(|(name, _, _)| name == "production-artifact-bytes")
        .expect("actual inner ScrollArea receiver drawn in same pass")
        .2
        .clone()
}
fn outer_offset(h: &Keyboard) -> egui::Vec2 {
    h.offsets()
        .into_iter()
        .find(|(name, _, _)| name == "production-script-workbench")
        .expect("actual outer ScrollArea state")
        .2
}

fn frame_meta(h: &Keyboard, out: &egui::FullOutput, domain: Rect) -> Value {
    let area = inner_area(h);
    json!({"pass":h.pass, "frame":h.flow.ctx.cumulative_frame_nr(),
        "inner_offset":format!("{:?}",inner_offset(h)),
        "outer_offset":format!("{:?}",outer_offset(h)),
        "oracle_domain":rect_value(domain), "current_area":rect_value(area.rect),
        "current_interact":rect_value(area.interact_rect),
        "repaint":format!("{:?}",out.viewport_output[&egui::ViewportId::ROOT].repaint_delay),
        "owner":format!("{:?}",h.flow.ctx.memory(|memory|memory.focused()))})
}
fn rect_value(rect: Rect) -> Value {
    json!([[rect.min.x, rect.min.y], [rect.max.x, rect.max.y]])
}
fn glyph_sample(glyph: &GlyphGeometry, domain: Rect, meta: &Value) -> (f32, Value) {
    let clip = glyph.clip.intersect(domain);
    let intersection = glyph.rect.intersect(glyph.clip);
    let fraction = (intersection.width().max(0.0) * intersection.height().max(0.0))
        / glyph.rect.area().max(f32::EPSILON);
    (
        fraction,
        json!({"index":glyph.index,"char":glyph.character.to_string(),
        "logical_rect":rect_value(glyph.rect),"row_rect":rect_value(glyph.row_rect),
        "actual_quad":glyph.quad.map(|quad|quad.map(|point|[point.x,point.y])),
        "actual_quad_fully_visible":glyph.quad.is_some_and(|quad|quad.iter().all(|point|clip.contains(*point))),
        "paint_clip":rect_value(glyph.clip),"effective_clip":rect_value(clip),
        "fully_visible":clip.contains_rect(glyph.rect),
        "paint_clip_fully_visible":glyph.clip.contains_rect(glyph.rect),"visible_fraction":fraction,
        "frame":meta}),
    )
}
