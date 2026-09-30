use super::*;
pub(super) use egui::{pos2, vec2, Event, MouseWheelUnit, RawInput, Rect};
use std::path::Path;
pub(super) const TEST_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0,
    5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
pub(super) fn point_canvas() -> MapCanvas {
    MapCanvas::new(MapRenderSnapshot {
        map_id: "map".into(),
        title: "测试地图".into(),
        extent: vec2(400.0, 400.0),
        canvas: core_canvas(400.0 as u32, 400.0 as u32),
        measurement: None,
        raster_layers: Vec::new(),
        layers: vec![MapLayer {
            id: "places".into(),
            title: "地点".into(),
            visible: true,
            locked: false,
            placements: vec![MapPlacement {
                id: "point".into(),
                target_ref: None,
                navigation: None,
                annotation: String::new(),
                role: String::new(),
                label_override: None,
                geometry: MapGeometry::Point(NormalizedPoint::new(0.25, 0.25)),
                style: MapStyle::default(),
            }],
        }],
    })
}
pub(super) fn test_workspace(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("worldedit-map-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".world")).expect("test workspace directory");
    std::fs::write(
            root.join("world.wl"),
            "world harbor as \"雾港\"\n  description \"地图阅读测试\"\nasset harbor_image image \"assets/harbor.png\" as \"港口图\"\n",
        )
        .expect("test source");
    std::fs::create_dir_all(root.join("assets")).expect("asset directory");
    std::fs::write(root.join("assets/harbor.png"), TEST_PNG).expect("test raster");
    std::fs::write(
        root.join(".world/project.json"),
        r#"{
  "schema_version": 1,
  "language_version": "1.9",
  "entry": "world.wl",
  "required_features": ["presentation.maps.v1"],
  "maps": {"harbor": ".world/maps/harbor.json"},
  "graph_views": {},
  "extensions": {}
}"#,
    )
    .expect("test manifest");
    std::fs::create_dir_all(root.join(".world/maps")).expect("map directory");
    std::fs::write(
            root.join(".world/maps/harbor.json"),
            r#"{
  "schema_version": 1,
  "id": "harbor",
  "title": "雾港地图",
  "raster_layers": [{"id": "base", "asset": {"kind": "asset", "id": "harbor_image"}, "rect": [0, 0, 1, 1]}],
  "canvas": {"width": 1000, "height": 600, "unit": "normalized"},
  "layer_order": ["places"],
  "layers": {"places": {"title": "地点", "visible_default": true, "locked": false}},
  "placements": {
    "lighthouse": {
      "layer_id": "places",
      "target_ref": {"kind": "world", "id": "harbor"},
      "geometry": {"kind": "point", "position": [0.4, 0.3]},
      "annotation": "查看灯塔",
      "role": "地点入口",
      "label_override": null,
      "navigation": null,
      "scope_refs": []
    }

  },
  "extensions": {}
}"#,
        )
        .expect("test map");
    root
}

pub(super) fn enable_entities(root: &Path) {
    let path = root.join(".world/project.json");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        path,
        text.replace(
            "\"language_version\": \"1.9\"",
            "\"language_version\": \"1.10\"",
        )
        .replace(
            "\"presentation.maps.v1\"",
            "\"presentation.maps.v1\", \"content.entities.v1\"",
        ),
    )
    .unwrap();
}

pub(super) fn navigation_workspace(name: &str) -> std::path::PathBuf {
    let root = test_workspace(name);
    std::fs::write(
        root.join(".world/project.json"),
        r#"{
  "schema_version": 1,
  "language_version": "1.9",
  "entry": "world.wl",
  "required_features": ["presentation.maps.v1"],
  "maps": {
    "harbor": ".world/maps/harbor.json",
    "city": ".world/maps/city.json"
  },
  "graph_views": {},
  "extensions": {}
}"#,
    )
    .expect("navigation manifest");
    let harbor_path = root.join(".world/maps/harbor.json");
    let harbor = std::fs::read_to_string(&harbor_path).expect("navigation harbor map");
    std::fs::write(
        harbor_path,
        harbor.replacen(
            r#""navigation": null"#,
            r#""navigation": {"map_id": "city"}"#,
            1,
        ),
    )
    .expect("navigation harbor entry");
    std::fs::write(
        root.join(".world/maps/city.json"),
        r#"{
  "schema_version": 1,
  "id": "city",
  "title": "城内地图",
  "raster_layers": [],
  "canvas": {"width": 800, "height": 500, "unit": "normalized"},
  "layer_order": ["places"],
  "layers": {"places": {"title": "地点", "visible_default": true, "locked": false}},
  "placements": {
    "harbor-return": {
      "layer_id": "places",
      "target_ref": null,
      "geometry": {"kind": "point", "position": [0.8, 0.7]},
      "annotation": "返回雾港",
      "role": "地图入口",
      "label_override": null,
      "navigation": {"map_id": "harbor"},
      "scope_refs": []
    }
  },
  "extensions": {}
}"#,
    )
    .expect("navigation child map");
    root
}

pub(super) fn click_canvas(
    ctx: &egui::Context,
    canvas: &mut MapCanvas,
    screen_rect: Rect,
    point: Pos2,
    time: f64,
) {
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            time: Some(time),
            events: vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            time: Some(time + 0.01),
            events: vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );
}

pub(super) fn core_canvas(width: u32, height: u32) -> worldline_core::presentation::MapCanvas {
    worldline_core::presentation::MapCanvas {
        width,
        height,
        unit: "normalized".into(),
        extra: Default::default(),
    }
}
