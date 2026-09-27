//! 可交互的时间线与事件关系图,结构和顺序均来自 core analysis。
mod canvas;
mod canvas_controls;
mod node;

pub(super) use node::{draw_node, NodeHeading};

const WIDTH: f32 = 228.0;
const HEIGHT: f32 = 106.0;
const CELL: f32 = 286.0;
const LANE: f32 = 178.0;
const LEFT: f32 = 142.0;
