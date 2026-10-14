use super::*;
use crate::theme::{Density, PaletteId, PaletteModeSupport, StylePreset, ThemeMode};

#[test]
fn normal_preview_keyboard_seventeen_members_and_layout_density_pairs_remain_visible() {
    let mut members = Vec::new();
    for palette in PaletteId::ALL {
        let modes: &[ThemeMode] = match palette.mode_support() {
            PaletteModeSupport::Both => &[ThemeMode::Light, ThemeMode::Dark],
            PaletteModeSupport::LightOnly => &[ThemeMode::Light],
            PaletteModeSupport::DarkOnly => &[ThemeMode::Dark],
        };
        for mode in modes {
            members.push((palette, *mode));
        }
    }
    assert_eq!(members.len(), 17);
    let styles = [
        StylePreset::Studio,
        StylePreset::Manuscript,
        StylePreset::Technical,
        StylePreset::Focus,
        StylePreset::Ledger,
    ];
    let densities = [Density::Compact, Density::Standard, Density::Spacious];
    for (n, (palette, mode)) in members.into_iter().enumerate() {
        let mut h = setup(Case::UpdateSay);
        let state = h.state();
        let fields = h.fields();
        let p = &mut h.app.personal.settings.appearance;
        p.palette = palette;
        p.theme = mode;
        p.style = styles[n % 5];
        p.density = densities[(n / 5) % 3];
        p.ui_scale = if n % 2 == 0 { 1.0 } else { 1.25 };
        p.body_size = if n % 3 == 0 { 24.0 } else { 17.0 };
        p.high_contrast = n % 4 == 0;
        h.size = [
            egui::vec2(1188.0, 848.0),
            egui::vec2(800.0, 600.0),
            egui::vec2(760.0, 720.0),
        ][n % 3];
        h.settle();
        h.tab_to(NORMAL_READ);
        let out = h.settle();
        assert!(h.reading_focus_visible(&out), "{palette:?} {mode:?} n={n}");
        let old = h.offset();
        h.key(Key::ArrowDown, Modifiers::NONE);
        assert!(
            h.offset() > old,
            "reading scroll must move at {palette:?} {mode:?}"
        );
        h.tab_to(STAGE);
        assert!(h.app.manuscript.pending_scroll.is_none());
        let mut out = h.key(Key::Tab, Modifiers::SHIFT);
        let owner = h
            .ctx
            .memory(|memory| memory.focused())
            .expect("reverse Tab has a control");
        let mut response = h
            .focused_response
            .clone()
            .expect("reverse Tab control is drawn");
        assert_eq!(response.id, owner, "same-pass reverse Tab owner");
        let domain = h.reading_clip();
        let before_repaint = (h.offset(), response.rect, response.interact_rect);
        assert!(h.app.manuscript.pending_scroll.is_none());
        if !domain.contains_rect(response.rect.expand(2.0))
            || !response.interact_rect.contains_rect(response.rect)
        {
            // Shift+Tab grants its owner on the key-up pass. ScrollArea builds
            // its content Ui before applying that pass's pending offset target.
            // Follow exactly the redraw requested by egui, with no new input.
            assert_eq!(
                out.viewport_output
                    .get(&egui::ViewportId::ROOT)
                    .expect("real root viewport output")
                    .repaint_delay,
                std::time::Duration::ZERO,
                "offscreen focus must have requested another real paint"
            );
            assert_eq!(h.state(), state);
            assert_eq!(h.fields(), fields);
            out = h.frame(vec![]);
            assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(owner));
            response = h.focused_response.clone().expect("redrawn actual owner");
            assert_eq!(response.id, owner, "requested repaint preserves owner");
            assert!(h.app.manuscript.pending_scroll.is_none());
        }
        assert!(
            domain.contains_rect(response.rect.expand(2.0))
                && response.interact_rect.contains_rect(response.rect)
                && texts(&out).iter().any(|(_, rect, clip)|
                    control_owns_label(&h.ctx, &response, *rect)
                        && domain.contains_rect(*rect)
                        && clip.contains_rect(*rect)),
            "reverse-Tab local control must be fully visible; member={palette:?}/{mode:?} n={n} domain={domain:?} before={before_repaint:?} after=({},{:?},{:?})",
            h.offset(), response.rect, response.interact_rect
        );
        assert!(
            has_control_focus_paint(&h.ctx, &out, response.rect),
            "member={palette:?}/{mode:?} n={n}; {}",
            focus_paint_evidence(&h, &out, &response)
        );
        assert_eq!(h.state(), state);
        assert_eq!(h.fields(), fields);
    }
}

// Failure evidence only; all observations occur after the captured paint-pass response.
fn focus_paint_evidence(h: &Harness, out: &egui::FullOutput, response: &egui::Response) -> String {
    let theme = crate::theme::resolved(&h.ctx);
    fn collect(
        shape: &egui::Shape,
        clip: Rect,
        target: Rect,
        focus: egui::Color32,
        rows: &mut Vec<String>,
    ) {
        match shape {
            egui::Shape::Rect(r)
                if r.stroke.width > 0.0
                    && (r.stroke.color == focus || r.rect.intersects(target)) =>
            {
                rows.push(format!(
                    "rect={:?} clip={clip:?} stroke={:?} fully_visible={}",
                    r.rect,
                    r.stroke,
                    clip.contains_rect(r.rect)
                ));
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, clip, target, focus, rows);
                }
            }
            _ => {}
        }
    }
    let mut rows = Vec::new();
    for shape in &out.shapes {
        collect(
            &shape.shape,
            shape.clip_rect,
            response.rect,
            theme.colors.focus,
            &mut rows,
        );
    }
    let labels: Vec<_> = texts(out)
        .into_iter()
        .filter(|(_, rect, _)| response.rect.contains(rect.center()))
        .collect();
    let layers = h.ctx.memory(|memory| memory.areas().visible_layer_ids());
    let modal = h.ctx.memory(|memory| memory.top_modal_layer());
    format!("style={:?} density={:?} scale={} viewport={:?} owner={:?} rect={:?} layer={:?} focus={} enabled={} labels={labels:?} expected=({:?},{}) popup={} modal={modal:?} visible_layers={layers:?} input_blocked={} strokes={rows:?}",
        theme.preferences.style, theme.preferences.density, theme.preferences.ui_scale, h.size, response.id, response.rect, response.layer_id, response.has_focus(), response.enabled(), theme.colors.focus, theme.focus_width,
        egui::Popup::is_any_open(&h.ctx), h.app.manuscript.writing_view.input_blocked(&h.ctx))
}
