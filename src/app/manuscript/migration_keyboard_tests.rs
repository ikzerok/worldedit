//! Real full-App keys and clipped paint geometry; not native or physical IME evidence.
use super::*;
use crate::app::{Tab, WorldeditApp};
use egui::{Event, Key, Modifiers, Rect, Vec2};
use serde_json::{json, Value};
mod appearance;
mod geometry;
mod guards;
mod harness;
mod pagination;
mod reading;
use geometry::*;
use harness::*;

const READ: &str = "迁移预览阅读区 · ↑↓滚动";
const ENABLE: &str = "明确预览启用语言 1.11；确认后与全文草稿一次应用";
const CONFIRM: &str = "已核对全稿解释变化、诊断与完整文件；确认一次应用";
const APPLY: &str = "确认迁移并应用所列完整稿";
const BODY: &str = "hello tide\nsecond line";
