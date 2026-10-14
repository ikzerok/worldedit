use super::*;
use egui::emath::GuiRounding;

fn domain(h: &Harness, out: &egui::FullOutput, owner: egui::Id) -> Rect {
    let r = h
        .focused_response
        .as_ref()
        .expect("current reader response");
    assert_eq!(r.id, owner);
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(owner));
    let theme = crate::theme::resolved(&h.ctx);
    fn visit(
        shape: &egui::Shape,
        clip: Rect,
        theme: &crate::theme::ResolvedTheme,
        owner: &egui::Response,
        result: &mut Vec<Rect>,
    ) {
        match shape {
            egui::Shape::Rect(rect) => {
                if rect.stroke == egui::Stroke::new(theme.focus_width, theme.colors.focus)
                    && clip.contains_rect(rect.rect)
                    && rect.rect.height() > owner.rect.height() + 2.0 * theme.focus_width
                    && (rect.rect.left() - clip.left()).abs() <= theme.focus_width
                    && (rect.rect.right() - clip.right()).abs() <= theme.focus_width
                    && (rect.rect.top() - clip.top()).abs() <= theme.focus_width
                    && (rect.rect.bottom() - clip.bottom()).abs() <= theme.focus_width
                {
                    result.push(clip);
                }
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    visit(shape, clip, theme, owner, result)
                }
            }
            _ => {}
        }
    }
    let mut domains = vec![];
    for shape in &out.shapes {
        visit(&shape.shape, shape.clip_rect, &theme, r, &mut domains)
    }
    assert_eq!(
        domains.len(),
        1,
        "one actual local reader frame, owner={owner:?}, domains={domains:?}"
    );
    domains[0]
}
fn values(h: &Harness) -> Vec<String> {
    let f = request(h);
    let DialogueOperation::Update { draft, .. } = f.operation else {
        panic!("Update")
    };
    let mut values = vec![];
    for part in draft.parts {
        values.push(match part {
            DialoguePart::Literal { text } => text,
            DialoguePart::Expression { source } => source,
            DialoguePart::Link { label, .. } => label,
        });
    }
    values.push(draft.direction.expect("original private direction"));
    values
}
fn observe(
    h: &Harness,
    out: &egui::FullOutput,
    domain: Rect,
    values: &[String],
    seen: &mut [BTreeSet<usize>],
) -> BTreeMap<(usize, usize), Rect> {
    let mut positions = BTreeMap::new();
    for (field, value) in values.iter().enumerate() {
        for (index, ch, visible, rect, _) in mesh::glyphs(&h.ctx, out, value, domain) {
            assert_eq!(
                value.chars().nth(index),
                Some(ch),
                "original UTF-8 field and real mesh agree"
            );
            if visible && !ch.is_whitespace() {
                seen[field].insert(index);
            }
            positions.insert((field, index), rect);
        }
    }
    positions
}
pub(super) fn all_fields(h: &mut Harness, require_overflow: bool) {
    let before = state(h);
    let out = seek(h, RECOVERY_READ);
    let owner = h.ctx.memory(|m| m.focused()).unwrap();
    let viewport = domain(h, &out, owner);
    let pixels_per_point = out.pixels_per_point;
    let stable_viewport = viewport.round_to_pixels(pixels_per_point);
    let label = visible_label(&out, RECOVERY_READ).unwrap();
    assert!(
        viewport.contains_rect(label),
        "reader name binds the actual local domain"
    );
    let values = values(h);
    let expected: Vec<BTreeSet<_>> = values
        .iter()
        .map(|value| {
            value
                .chars()
                .enumerate()
                .filter_map(|(i, c)| (!c.is_whitespace()).then_some(i))
                .collect()
        })
        .collect();
    let mut seen = vec![BTreeSet::new(); values.len()];
    let mut positions = observe(h, &out, viewport, &values, &mut seen);
    let mut down_moved = false;
    for _ in 0..360 {
        if seen == expected {
            break;
        }
        let out = key(h, Key::ArrowDown, Modifiers::NONE).0;
        let current_domain = domain(h, &out, owner);
        assert_eq!(out.pixels_per_point, pixels_per_point);
        // ScrollArea's relative->screen arithmetic may differ by a subpixel ULP.
        // Only cross-frame footprint identity uses egui's exact device-pixel grid.
        assert_eq!(
            current_domain.round_to_pixels(pixels_per_point),
            stable_viewport,
            "stable reading footprint: initial_bits={:?} current_bits={:?}",
            rect_bits(viewport),
            rect_bits(current_domain)
        );
        // Ink is always checked against this pass's original unrounded clip/domain.
        let next = observe(h, &out, current_domain, &values, &mut seen);
        down_moved |= next
            .iter()
            .any(|(k, r)| positions.get(k).is_some_and(|old| old.top() > r.top()));
        positions = next;
        assert_eq!(state(h), before);
    }
    assert_eq!(
        seen, expected,
        "every non-whitespace glyph of each exact original field must be fully readable"
    );
    if require_overflow {
        assert!(
            down_moved,
            "real Down moves actual same-field ink, not unrelated App text"
        );
    }
    let mut up_moved = false;
    let mut top_seen = vec![BTreeSet::new(); values.len()];
    for _ in 0..if down_moved { 360 } else { 1 } {
        let out = key(h, Key::ArrowUp, Modifiers::NONE).0;
        let current_domain = domain(h, &out, owner);
        assert_eq!(out.pixels_per_point, pixels_per_point);
        assert_eq!(
            current_domain.round_to_pixels(pixels_per_point),
            stable_viewport,
            "stable Up footprint: initial_bits={:?} current_bits={:?}",
            rect_bits(viewport),
            rect_bits(current_domain)
        );
        let next = observe(h, &out, current_domain, &values, &mut top_seen);
        up_moved |= next
            .iter()
            .any(|(k, r)| positions.get(k).is_some_and(|old| old.top() < r.top()));
        positions = next;
        assert_eq!(state(h), before);
        if top_seen[0] == expected[0] {
            break;
        }
    }
    assert!(!down_moved || up_moved);
    assert_eq!(
        top_seen[0], expected[0],
        "Up can read the full original first field again"
    );
    let out = key(h, Key::Escape, Modifiers::NONE).0;
    let out = reveal(h, out);
    focused_control(h, &out, COPY);
    assert!(
        !out.platform_output
            .commands
            .iter()
            .any(|c| matches!(c, egui::OutputCommand::CopyText(_))),
        "Escape focuses Copy without executing it"
    );
    assert_eq!(state(h), before);
}

fn rect_bits(rect: Rect) -> [u32; 4] {
    [
        rect.min.x.to_bits(),
        rect.min.y.to_bits(),
        rect.max.x.to_bits(),
        rect.max.y.to_bits(),
    ]
}
