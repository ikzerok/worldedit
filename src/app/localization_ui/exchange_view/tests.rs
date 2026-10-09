use super::*;
use crate::app::localization_ui::workbench_tests::fixture;
use egui::{Event, Pos2};

struct Harness {
    ctx: egui::Context,
    exchange: LocalizationExchange,
    plan: LocalizationImportPlan,
    export: LocalizationExportPlan,
    view: View,
    navigation: Option<navigation::Request>,
    time: f64,
}

impl Harness {
    fn new(count: usize, long: bool) -> Self {
        let (project, mut state) = fixture(count);
        state.string_ids = (0..count)
            .map(|index| format!("line{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let selection = state.selection();
        let export = project.preview_localization_export(&selection).unwrap();
        assert!(export.can_export);
        let mut exchange = export.exchange.clone();
        for entry in &mut exchange.entries {
            entry.translation_parts = Some(entry.source_parts.clone());
        }
        if long {
            exchange.entries[0].translation_parts = Some(vec![LocalizationPart::Text {
                text: format!("{}最后段标记😀", "长😀".repeat(8000)),
            }]);
        }
        let plan = project
            .preview_localization_import_candidate(&selection, &exchange)
            .unwrap();
        assert!(plan.can_apply);
        Self {
            ctx: egui::Context::default(),
            exchange,
            plan,
            export,
            view: View::default(),
            navigation: None,
            time: 0.0,
        }
    }

    fn frame(&mut self, events: Vec<Event>, export: bool) -> egui::FullOutput {
        self.time += 1.0 / 60.0;
        self.ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(1680.0, 8000.0),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if export {
                        exchange::show_export_plan(
                            ui,
                            &self.export,
                            &mut self.navigation,
                            &mut self.view,
                        );
                    } else {
                        exchange::show_exchange_entries(
                            ui,
                            &self.exchange,
                            &mut self.navigation,
                            &self.plan,
                            &mut self.view,
                        );
                    }
                });
            },
        )
    }

    fn click(&mut self, label: &str, last: bool) {
        self.frame(vec![], false);
        let output = self.frame(vec![], false);
        let positions: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    let rect = text
                        .galley
                        .rect
                        .translate(text.pos.to_vec2())
                        .intersect(shape.clip_rect);
                    rect.is_positive().then(|| rect.center())
                }
                _ => None,
            })
            .collect();
        let point = if last {
            positions.last()
        } else {
            positions.first()
        }
        .copied()
        .unwrap_or_else(|| panic!("visible {label}"));
        for pressed in [true, false] {
            self.frame(
                vec![
                    Event::PointerMoved(point),
                    Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                false,
            );
        }
    }
}

fn text(output: &egui::FullOutput) -> String {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.text()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn localization_exchange_thousand_entries_pages_every_identity_without_rendering_or_truncating_all()
{
    let mut h = Harness::new(1000, false);
    let before = serde_json::to_vec(&h.exchange).unwrap();
    let ids: std::collections::BTreeSet<_> = h
        .exchange
        .entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect();
    let mut visited = Vec::new();
    for page in 0..25 {
        let output = h.frame(vec![], false);
        assert_eq!(h.view.offset, page * PAGE_SIZE);
        let rendered: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if ids.contains(text.galley.text()) => {
                    Some(text.galley.text().to_owned())
                }
                _ => None,
            })
            .collect();
        let expected: Vec<_> = h.exchange.entries[page * PAGE_SIZE..(page + 1) * PAGE_SIZE]
            .iter()
            .map(|entry| entry.id.clone())
            .collect();
        assert_eq!(rendered, expected);
        assert!(text(&output).contains("/ 1000 项 · 每页 40 项"));
        assert!(text(&output).len() < json_input::EDIT_BYTES * 2);
        visited.extend(rendered);
        let source = h.exchange.entries[page * PAGE_SIZE].source.clone();
        h.click(
            &format!("定位来源 {}:{} · {}", source.file, source.line, source.kind),
            false,
        );
        assert_eq!(h.navigation.take().unwrap().source, source);
        h.click("结果下一页", false);
    }
    assert_eq!(
        h.view.offset, 960,
        "last page cannot advance past the exact total"
    );
    assert_eq!(
        visited
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        ids
    );
    h.click("结果上一页", false);
    assert_eq!(h.view.offset, 920);
    assert_eq!(serde_json::to_vec(&h.exchange).unwrap(), before);
    let output = h.frame(vec![], true);
    assert_eq!(
        h.view.offset, 0,
        "a different accepted plan owns independent display identity"
    );
    assert!(text(&output).contains("显示 1–40 / 1000 项"));
    assert!(text(&output).len() < json_input::EDIT_BYTES * 2);
    assert_eq!(h.export.exchange.entries.len(), 1000);
}

#[test]
fn localization_exchange_explicit_long_detail_reaches_later_utf8_segments_without_mutating_input() {
    let mut h = Harness::new(1, true);
    let before = serde_json::to_vec(&h.exchange).unwrap();
    let default = h.frame(vec![], false);
    assert!(!text(&default).contains("最后段标记😀"));
    assert!(h.view.selected.is_none());
    h.click("查看此项详情", false);
    assert_eq!(h.view.selected, Some(0));
    let first = h.frame(vec![], false);
    assert!(!text(&first).contains("最后段标记😀"));
    for _ in 0..3 {
        h.click("详情下一段", true);
    }
    let last = h.frame(vec![], false);
    assert!(text(&last).contains("最后段标记😀"));
    assert!(h.view.translation_start >= json_input::EDIT_BYTES * 2);
    assert!(text(&last).len() < json_input::EDIT_BYTES * 2);
    h.click("详情首段", true);
    assert_eq!(h.view.translation_start, 0);
    h.click("详情末段", true);
    assert!(text(&h.frame(vec![], false)).contains("最后段标记😀"));
    h.click("收起此项详情", false);
    assert!(h.view.selected.is_none());
    assert_eq!(serde_json::to_vec(&h.exchange).unwrap(), before);
}

#[test]
fn localization_exchange_part_slices_preserve_all_roles_and_utf8_with_a_fixed_layout_budget() {
    let parts = vec![
        LocalizationPart::Text {
            text: "中文😀".repeat(9000),
        },
        LocalizationPart::Placeholder {
            token: "{count}".into(),
        },
        LocalizationPart::Link {
            token: "entity:人物".into(),
            label: "标签😀".repeat(4000),
        },
    ];
    let original: String = fragments(&parts).collect();
    let mut start = 0;
    let mut rendered = String::new();
    while start < original.len() {
        let piece = slice(&parts, start);
        assert!(piece.text.len() <= json_input::EDIT_BYTES);
        assert!(original.is_char_boundary(piece.start) && original.is_char_boundary(piece.end));
        rendered.push_str(&piece.text);
        start = piece.end;
    }
    assert_eq!(rendered, original);
    let empty_parts = vec![
        LocalizationPart::Text {
            text: String::new()
        };
        4096
    ];
    let bounded = slice(&empty_parts, 0);
    assert!(bounded.total > json_input::EDIT_BYTES);
    assert!(!bounded.text.is_empty() && bounded.text.len() <= json_input::EDIT_BYTES);
    assert!(summary(&empty_parts).chars().count() <= 89);
}
