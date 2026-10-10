use super::*;
use egui::emath::GuiRounding;

pub(super) type Glyph = (usize, char, bool, Rect, Rect);
pub(super) fn glyphs(
    ctx: &egui::Context,
    out: &egui::FullOutput,
    page: &str,
    domain: Rect,
) -> Vec<Glyph> {
    glyph_geometry(ctx, out, page)
        .into_iter()
        .map(|g| {
            let clip = g.clip.intersect(domain);
            let visible = g
                .quad
                .is_some_and(|quad| quad.iter().all(|p| clip.contains(*p)));
            (g.index, g.character, visible, g.rect, clip)
        })
        .collect()
}

#[derive(Clone)]
struct GlyphGeometry {
    index: usize,
    character: char,
    rect: Rect,
    clip: Rect,
    quad: Option<[egui::Pos2; 4]>,
}
/// Bind every glyph index to the actual pre-tessellated mesh vertices, using
/// the same galley pixel rounding and row/angle transform as epaint::Tessellator.
/// Logical font metrics remain separate diagnostic data, never a relaxed epsilon.
fn glyph_geometry(ctx: &egui::Context, out: &egui::FullOutput, page: &str) -> Vec<GlyphGeometry> {
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
