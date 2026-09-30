//! 真实 egui 窗口按钮回归；不是已显示桌面或浏览器的人工验收。
use super::{Pending, SavedReplayPath, Tab, WorldeditApp};
use egui::{pos2, vec2, Event, PointerButton, RawInput, Rect};
use std::sync::atomic::Ordering;
use worldline_core::{project::Project, TargetRef};

mod fixtures;
mod interaction;
mod rendering;

use self::fixtures::*;
use self::interaction::*;
use self::rendering::*;

mod authoring_forms;
mod authoring_mentions;
mod authoring_source;
mod catalog;
mod checkpoint;
mod debugger;
mod import;
mod localization;
mod manuscript;
mod network;
mod reader_publish;
mod reading;
mod review;
mod templates;
mod topic_views;

mod source_gutter;

mod source_focus;

mod workspace_refresh;

mod keyboard;
