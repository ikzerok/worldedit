use crate::app::WorldeditApp;
use std::path::PathBuf;
use std::sync::Arc;
use worldline_core::manuscript::{
    ManuscriptDeliveryReport, ManuscriptDeliveryRequest, ManuscriptQueryRequest,
    ManuscriptQuerySnapshot,
};

#[derive(Default)]
pub(super) struct State {
    pub reviewed: Option<Arc<ManuscriptDeliveryReport>>,
    pub captured: Option<Input>,
    pub job: Option<super::delivery_job::Job>,
    pub notice: Option<String>,
    pub page: usize,
    pub review_pages: Vec<std::ops::Range<usize>>,
    pub markdown_open: bool,
    pub markdown_page: usize,
    pub confirmed: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub destination: String,
    pub excluded_inputs: Vec<String>,
    pub action: Option<Action>,
}
pub(super) enum Action {
    Copy,
    Save,
    #[cfg(not(target_arch = "wasm32"))]
    Browse,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Input {
    pub version: u64,
    pub snapshot: String,
    pub request: ManuscriptDeliveryRequest,
    pub buffers: Vec<(PathBuf, u64, bool)>,
}

impl WorldeditApp {
    pub(in crate::app::manuscript) fn verify_scoped_review_observation(
        &self,
    ) -> Result<(), String> {
        let snapshot = self.manuscript_delivery_snapshot()?;
        self.project
            .verify_manuscript_delivery_observation(&snapshot)
            .map_err(|error| error.to_string())
    }
    pub(in crate::app::manuscript) fn manuscript_scoped_review_is_current(
        &self,
        request: &super::super::review_navigation::ReviewRequest,
    ) -> bool {
        self.manuscript_delivery_is_current()
            && self
                .manuscript
                .preview_cache
                .delivery
                .reviewed
                .as_ref()
                .is_some_and(|report| {
                    request.key == report.scope().snapshot_key
                        && report.contains_review(&request.review)
                })
    }
    pub(in crate::app) fn manuscript_delivery_request(&self) -> Option<ManuscriptDeliveryRequest> {
        let id = self.manuscript.selected_book.as_ref()?;
        let session = &self.manuscript.navigation.session;
        let mut request = ManuscriptDeliveryRequest::new(ManuscriptQueryRequest {
            manuscript_id: id.clone(),
            text: session.text.clone(),
            status: session.status.clone(),
            pov: session.pov.clone(),
            section_id: session.section_id.clone(),
            ..Default::default()
        });
        request.expected_snapshot_key = self
            .manuscript
            .preview_cache
            .query_snapshot
            .as_ref()
            .map(|snapshot| snapshot.key().into());
        Some(request)
    }

    pub(super) fn manuscript_delivery_input(&self) -> Option<Input> {
        let snapshot = self.manuscript.preview_cache.query_snapshot.as_ref()?;
        Some(Input {
            version: self.version,
            snapshot: snapshot.key().into(),
            request: self.manuscript_delivery_request()?,
            // 与查询/交付core一致：只读打开的原稿缓冲不是新增正文草稿输入。
            buffers: self
                .manuscript
                .writing_buffer_stamps()
                .into_iter()
                .filter(|(_, _, changed)| *changed)
                .collect(),
        })
    }

    pub(in crate::app) fn manuscript_delivery_is_current(&self) -> bool {
        let state = &self.manuscript.preview_cache.delivery;
        state.job.is_none()
            && state
                .captured
                .as_ref()
                .zip(self.manuscript_delivery_input().as_ref())
                .is_some_and(|(old, current)| old == current)
            && state.reviewed.as_ref().is_some_and(|report| {
                self.manuscript
                    .preview_cache
                    .query_snapshot
                    .as_ref()
                    .is_some_and(|snapshot| report.matches_snapshot(snapshot))
            })
    }

    pub(super) fn manuscript_delivery_snapshot(
        &self,
    ) -> Result<Arc<ManuscriptQuerySnapshot>, String> {
        self.manuscript
            .preview_cache
            .query_snapshot
            .clone()
            .ok_or_else(|| "当前筛选快照未就绪；没有借用旧范围".into())
    }
}
