//! Full-App, normal-size keyboard reachability. Pointer/wheel setup is explicitly
//! separate: it establishes independent real UI starting points, not keyboard proof.
mod delivery;
mod geometry;
mod harness;
mod queries;
mod reading;
mod setup;
mod states;
use super::*;
use egui::{Event, Key, Modifiers, Rect};
use geometry::*;
use harness::Keyboard;
const ROLE_HINT: &str = "留空为全部，例如 traveler";
const LOCALE_HINT: &str = "留空为源文";
const PATH_HINT: &str = "工作区外新文件完整路径";
const DIRECTION: &str = "明确纳入作者私密演出备注（源语言）";
