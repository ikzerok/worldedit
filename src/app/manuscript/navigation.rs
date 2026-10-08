//! 个人章节导航状态；查询语义、层级与分页来自 core。
use super::Layout;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use worldline_core::manuscript::{
    ManuscriptQueryPage, ManuscriptQueryRequest, ManuscriptQuerySnapshot,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(in crate::app) struct Columns {
    pub identity: bool,
    pub summary: bool,
    pub perspective: bool,
    pub status: bool,
    pub statistics: bool,
    pub goal: bool,
    pub source: bool,
}

impl Default for Columns {
    fn default() -> Self {
        Self {
            identity: false,
            summary: true,
            perspective: true,
            status: true,
            statistics: true,
            goal: true,
            source: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(in crate::app) struct NavigationSession {
    pub text: String,
    pub status: String,
    pub pov: String,
    pub section_id: Option<String>,
    pub offset: usize,
    pub scroll_y: f32,
    pub columns: Columns,
    pub layout: Layout,
    pub collapsed: Vec<String>,
}

#[derive(Default)]
pub(super) struct NavigationState {
    pub input: super::navigation_input::Composition,
    pub session: NavigationSession,
    pub pending_scroll: Option<f32>,
    pub enter_editor: bool,
    pub request_row_focus: bool,
    pub diagnostic_offset: usize,
    page_key: String,
    page: Option<Result<Arc<ManuscriptQueryPage>, String>>,
}

impl NavigationState {
    pub(super) fn invalidate(&mut self) {
        self.page_key.clear();
        self.page = None;
    }
    pub(super) fn reset_page(&mut self) {
        self.session.offset = 0;
        self.pending_scroll = Some(0.0);
    }

    pub(super) fn restore(&mut self, session: NavigationSession) {
        self.pending_scroll = Some(if session.scroll_y.is_finite() {
            session.scroll_y.clamp(0.0, 1_000_000.0)
        } else {
            0.0
        });
        self.session = session;
        self.page_key.clear();
        self.page = None;
    }

    pub(super) fn query(
        &mut self,
        snapshot: &ManuscriptQuerySnapshot,
        request: &ManuscriptQueryRequest,
    ) -> Result<Arc<ManuscriptQueryPage>, String> {
        let key = format!(
            "{}|{}",
            snapshot.key(),
            serde_json::to_string(request).expect("章节查询只含可序列化的 DTO")
        );
        if self.page_key != key || self.page.is_none() {
            self.page = Some(
                snapshot
                    .query(request)
                    .map(Arc::new)
                    .map_err(|e| e.to_string()),
            );
            self.page_key = key;
        }
        self.page.as_ref().expect("已建立查询结果").clone()
    }
}
