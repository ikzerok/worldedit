use super::*;
use egui::emath::GuiRounding;

pub(super) type GlyphPage = (Rect, Vec<(usize, char, bool)>);

pub(super) fn texts(out: &egui::FullOutput) -> Vec<(String, Rect, Rect)> {
    fn visit(shape: &egui::Shape, clip: Rect, rows: &mut Vec<(String, Rect, Rect)>) {
        match shape {
            egui::Shape::Text(t) => rows.push((
                t.galley.text().into(),
                t.galley.rect.translate(t.pos.to_vec2()),
                clip,
            )),
            egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, clip, rows)),
            _ => {}
        }
    }
    let mut rows = vec![];
    for s in &out.shapes {
        visit(&s.shape, s.clip_rect, &mut rows);
    }
    rows
}
pub(super) fn visible(out: &egui::FullOutput, label: &str) -> Option<(Rect, Rect)> {
    texts(out).into_iter().find_map(|(text, rect, clip)| {
        (text == label && clip.contains_rect(rect)).then_some((rect, clip))
    })
}
pub(super) fn owns(ctx: &egui::Context, response: &egui::Response, label: Rect) -> bool {
    let style = ctx.style();
    response.sense.senses_click()
        && !response.sense.senses_drag()
        && response.rect.expand(1.0).contains_rect(label)
        && response.rect.height()
            <= (label.height() + style.spacing.button_padding.y * 2.0)
                .max(style.spacing.interact_size.y)
                + 2.0
}
pub(super) fn focus_paint(
    ctx: &egui::Context,
    out: &egui::FullOutput,
    rect: Rect,
    primary: bool,
) -> bool {
    let theme = crate::theme::resolved(ctx);
    let color = if primary {
        theme.colors.on_accent
    } else {
        theme.colors.focus
    };
    fn visit(
        shape: &egui::Shape,
        clip: Rect,
        target: Rect,
        color: egui::Color32,
        width: f32,
    ) -> bool {
        match shape {
            egui::Shape::Rect(r) => {
                r.stroke.color == color
                    && r.stroke.width >= width
                    && clip.contains_rect(r.rect)
                    && r.rect.contains(target.center())
                    && (r.rect.width() - target.width()).abs() <= 8.0
                    && (r.rect.height() - target.height()).abs() <= 8.0
            }
            egui::Shape::Vec(v) => v.iter().any(|s| visit(s, clip, target, color, width)),
            _ => false,
        }
    }
    out.shapes
        .iter()
        .any(|s| visit(&s.shape, s.clip_rect, rect, color, theme.focus_width))
}
/// Exact page text establishes byte identity. Non-whitespace glyph indices let a
/// long unwrapped row be read across real horizontal positions, without requiring
/// an impossible simultaneous full-width row fit or changing font/wrap settings.
pub(super) fn glyphs(ctx: &egui::Context, out: &egui::FullOutput, page: &str) -> Option<GlyphPage> {
    let glyphs = glyph_geometry(ctx, out, page);
    let clip = glyphs.first()?.clip;
    Some((
        clip,
        glyphs
            .into_iter()
            .map(|glyph| {
                let visible = glyph
                    .quad
                    .is_some_and(|quad| quad.iter().all(|point| clip.contains(*point)));
                (glyph.index, glyph.character, visible)
            })
            .collect(),
    ))
}

pub(super) fn checkbox_focus_paint(
    ctx: &egui::Context,
    out: &egui::FullOutput,
    response: &egui::Response,
) -> bool {
    let theme = crate::theme::resolved(ctx);
    let width = ctx.style().spacing.icon_width;
    // egui's standard Checkbox paints the focused icon, not a new full-label
    // outline. Bind its exact icon size/left position to this real Checkbox owner.
    fn visit(
        s: &egui::Shape,
        clip: Rect,
        r: Rect,
        width: f32,
        color: egui::Color32,
        stroke: f32,
    ) -> bool {
        match s {
            egui::Shape::Rect(p) => {
                p.stroke.color == color
                    && p.stroke.width >= stroke
                    && clip.contains_rect(p.rect)
                    && r.contains_rect(p.rect)
                    && (p.rect.width() - width).abs() <= 1.0
                    && (p.rect.height() - width).abs() <= 1.0
                    && (p.rect.left() - r.left()).abs() <= 1.0
            }
            egui::Shape::Vec(v) => v.iter().any(|s| visit(s, clip, r, width, color, stroke)),
            _ => false,
        }
    }
    out.shapes.iter().any(|s| {
        visit(
            &s.shape,
            s.clip_rect,
            response.rect,
            width,
            theme.colors.focus,
            theme.focus_width,
        )
    })
}

/// TextEdit has its own egui contract: its focused frame uses selection.stroke,
/// not Button's active.bg_stroke. Keep exact owner, frame and clipping checks.
pub(super) fn text_focus_paint(
    ctx: &egui::Context,
    out: &egui::FullOutput,
    response: &egui::Response,
) -> bool {
    let stroke = ctx.style().visuals.selection.stroke;
    let target = response
        .rect
        .expand(ctx.style().visuals.widgets.active.expansion);
    fn visit(s: &egui::Shape, clip: Rect, target: Rect, stroke: egui::Stroke) -> bool {
        match s {
            egui::Shape::Rect(p) => {
                p.stroke == stroke
                    && clip.contains_rect(p.rect)
                    && (p.rect.min - target.min).length() <= 0.5
                    && (p.rect.max - target.max).length() <= 0.5
            }
            egui::Shape::Vec(v) => v.iter().any(|s| visit(s, clip, target, stroke)),
            _ => false,
        }
    }
    response.has_focus()
        && ctx.memory(|m| m.focused()) == Some(response.id)
        && egui::TextEdit::load_state(ctx, response.id).is_some()
        && response.interact_rect.contains_rect(response.rect)
        && out
            .shapes
            .iter()
            .any(|s| visit(&s.shape, s.clip_rect, target, stroke))
}

pub(super) fn glyphs_in(
    ctx: &egui::Context,
    out: &egui::FullOutput,
    page: &str,
    domain: Rect,
) -> Option<GlyphPage> {
    let mut clipped = out.clone();
    for shape in &mut clipped.shapes {
        shape.clip_rect = shape.clip_rect.intersect(domain);
    }
    glyphs(ctx, &clipped, page)
}

#[derive(Clone)]
pub(super) struct GlyphGeometry {
    pub index: usize,
    pub character: char,
    pub rect: Rect,
    pub row_rect: Rect,
    pub clip: Rect,
    pub quad: Option<[egui::Pos2; 4]>,
}
/// Bind every glyph index to the actual pre-tessellated mesh vertices, using
/// the same galley pixel rounding and row/angle transform as epaint::Tessellator.
/// Logical font metrics remain separate diagnostic data, never a relaxed epsilon.
pub(super) fn glyph_geometry(
    ctx: &egui::Context,
    out: &egui::FullOutput,
    page: &str,
) -> Vec<GlyphGeometry> {
    fn visit(
        shape: &egui::Shape,
        clip: Rect,
        page: &str,
        transform: (f32, bool),
        result: &mut Vec<GlyphGeometry>,
    ) {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == page => {
                let mut index = 0;
                let origin = if transform.1 {
                    text.pos.round_to_pixels(transform.0)
                } else {
                    text.pos
                };
                let rotation = egui::emath::Rot2::from_angle(text.angle);
                for row in &text.galley.rows {
                    let range = row.visuals.glyph_vertex_range.clone();
                    let mut vertex = range.start;
                    let row_origin = origin + rotation * row.pos.to_vec2();
                    for glyph in &row.glyphs {
                        let quad = if glyph.uv_rect.is_nothing() {
                            assert!(
                                glyph.chr.is_whitespace(),
                                "non-whitespace glyph must have actual mesh: {index} {:?}",
                                glyph.chr
                            );
                            None
                        } else {
                            assert!(
                                vertex + 4 <= range.end,
                                "glyph must own exactly four vertices"
                            );
                            let vertices = &row.visuals.mesh.vertices[vertex..vertex + 4];
                            let actual_uv = Rect::from_points(
                                &vertices.iter().map(|v| v.uv).collect::<Vec<_>>(),
                            );
                            let uv = glyph.uv_rect;
                            assert_eq!(
                                actual_uv,
                                Rect::from_min_max(
                                    egui::pos2(f32::from(uv.min[0]), f32::from(uv.min[1])),
                                    egui::pos2(f32::from(uv.max[0]), f32::from(uv.max[1]))
                                ),
                                "mesh quad and exact character UV must correspond one-to-one"
                            );
                            let quad: [egui::Pos2; 4] = std::array::from_fn(|n| {
                                row_origin + rotation * vertices[n].pos.to_vec2()
                            });
                            vertex += 4;
                            Some(quad)
                        };
                        result.push(GlyphGeometry {
                            index,
                            character: glyph.chr,
                            rect: glyph
                                .logical_rect()
                                .translate(text.pos.to_vec2() + row.pos.to_vec2()),
                            row_rect: row.rect().translate(text.pos.to_vec2()),
                            clip,
                            quad,
                        });
                        index += 1;
                    }
                    assert_eq!(
                        vertex, range.end,
                        "every real glyph mesh vertex was matched exactly once"
                    );
                    if row.ends_with_newline {
                        index += 1;
                    }
                }
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    visit(shape, clip, page, transform, result);
                }
            }
            _ => {}
        }
    }
    let mut result = vec![];
    let transform = (
        out.pixels_per_point,
        ctx.tessellation_options(|options| options.round_text_to_pixels),
    );
    for shape in &out.shapes {
        visit(&shape.shape, shape.clip_rect, page, transform, &mut result);
    }
    result
}
