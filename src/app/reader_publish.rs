//! 显式选择并交付静态读者包；分析、过滤和 HTML 渲染都由 worldline-core 完成。

#[cfg(not(target_arch = "wasm32"))]
mod browser_preview;
mod close;
mod delivery;
mod maps;
#[cfg(not(target_arch = "wasm32"))]
mod native_delivery;
mod page_directory;
mod page_review;
mod preview;
mod profile_commit;
mod profile_job;
mod profiles;
mod review_ui;
mod selection;
mod selection_dto;
mod selection_ui;
mod unavailable;
#[cfg(target_arch = "wasm32")]
mod web_job;
#[cfg(target_arch = "wasm32")]
mod web_profile_job;
#[cfg(target_arch = "wasm32")]
mod web_snapshot;
mod window;

use crate::archive;
use std::collections::{BTreeMap, BTreeSet};
use worldline_core::catalog::TargetRef;
use worldline_core::reader_export::{
    ReaderExportPreview, ReaderExportSelection, ReaderProfileMigrationPlan,
    ReaderPublicationProfile,
};

#[derive(Default)]
pub(super) struct ReaderPublishState {
    pub(super) open: bool,
    initialized: bool,
    site_title: String,
    step: PublishStep,
    group: SelectionGroup,
    query: String,
    kind_filter: String,
    object_page: usize,
    map_page: usize,
    unavailable_page: usize,
    page_directory: page_directory::PageDirectory,
    resource_page: usize,
    files_page: usize,
    profiles: Vec<ReaderPublicationProfile>,
    profile: Option<ReaderPublicationProfile>,
    profile_id: String,
    profile_title: String,
    profile_error: Option<String>,
    profile_job: Option<profile_job::ProfileJob>,
    profile_plan: Option<(
        profile_job::ProfileInput,
        worldline_core::reader_export::ReaderProfileSavePlan,
    )>,
    migration: Option<ReaderProfileMigrationPlan>,
    migration_origin: Option<ReaderPublicationProfile>,
    story_details: bool,
    generation: u64,
    object_choices: Vec<ObjectChoice>,
    manuscript_choices: Vec<ManuscriptChoice>,
    attachment_choices: Vec<AttachmentChoice>,
    objects: BTreeSet<TargetRef>,
    fields: BTreeMap<TargetRef, BTreeSet<String>>,
    map_choices: Vec<MapChoice>,
    maps: BTreeMap<String, worldline_core::reader_export::ReaderMapSelection>,
    chapters: BTreeMap<String, BTreeSet<String>>,
    attachments: BTreeSet<String>,
    #[cfg(not(target_arch = "wasm32"))]
    destination: String,
    reviewed: Option<ReviewedPackage>,
    confirmed: bool,
    status: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    job: Option<ReaderPublishJob>,
    #[cfg(not(target_arch = "wasm32"))]
    delivery_job: Option<native_delivery::NativeDeliveryJob>,
    #[cfg(not(target_arch = "wasm32"))]
    browser_preview_job: Option<browser_preview::BrowserPreviewJob>,
    #[cfg(not(target_arch = "wasm32"))]
    close_drain: Option<close::CloseDrain>,
    #[cfg(target_arch = "wasm32")]
    web_job: Option<web_job::WebReaderJob>,
}

struct ReviewedPackage {
    selection: ReaderExportSelection,
    profile: Option<ReaderPublicationProfile>,
    preview: ReaderExportPreview,
    files: std::sync::Arc<archive::Files>,
    zip: std::sync::Arc<Vec<u8>>,
    raw_bytes: usize,
}

#[cfg(not(target_arch = "wasm32"))]
struct ReaderPublishJob {
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    receiver: Option<std::sync::mpsc::Receiver<ReaderPublishMessage>>,
    generation: u64,
}

#[cfg(not(target_arch = "wasm32"))]
enum ReaderPublishMessage {
    Stage(String),
    Done(Box<Result<ReviewedPackage, String>>),
}

#[derive(Clone, PartialEq, Eq)]
struct ManuscriptChoice {
    id: String,
    title: String,
    chapters: Vec<(String, String)>,
    unavailable: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
struct ObjectChoice {
    fields: Vec<worldline_core::reader_export::ReaderFieldCandidate>,
    target: TargetRef,
    display: String,
    aliases: Vec<String>,
}

#[derive(Clone, PartialEq, Eq)]
struct AttachmentChoice {
    id: String,
    display: String,
    available: bool,
}

enum PublishAction {
    Preview(ReaderExportSelection),
    Cancel,
    Publish,
    LoadProfile(usize),
    NewProfile,
    SaveProfile,
    PreviewMigration,
    ConfirmMigration,
    CancelMigration,
    #[cfg(not(target_arch = "wasm32"))]
    Browse,
    #[cfg(not(target_arch = "wasm32"))]
    BrowserPreview(String),
}

#[derive(Clone)]
struct MapChoice {
    id: String,
    title: String,
    placements: Vec<(String, String)>,
    rasters: Vec<(String, String)>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum PublishStep {
    #[default]
    Select,
    Resources,
    Preview,
    Generate,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum SelectionGroup {
    #[default]
    Objects,
    Maps,
    Chapters,
    Attachments,
}

impl ReaderPublishState {
    pub(super) fn busy(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.profile_job.is_some()
                || self.job.is_some()
                || self.delivery_job.is_some()
                || self.browser_preview_job.is_some()
                || self.close_drain.is_some()
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.profile_job.is_some() || self.web_job.is_some()
        }
    }

    fn matches_review(&self, reviewed: &ReviewedPackage, baseline: &str) -> bool {
        reviewed.preview.content_baseline == baseline
            && reviewed.selection == self.selection()
            && reviewed.profile == self.current_profile()
    }
}
