//! 只索引本次逐文件验证后的公开投影；路径是页面身份，数组下标只用于分页。
use worldline_core::reader_export::ReaderExportPreview;

mod ui;

const PAGE_SIZE: usize = 12;

#[derive(Default)]
pub(super) struct PageDirectory {
    pub(super) visible: bool,
    pub(super) query: String,
    pub(super) opened_path: Option<String>,
    bundle: Option<(String, String)>,
    cached_query: Option<String>,
    index: Vec<SearchPage>,
    matches: Vec<usize>,
    page: usize,
    candidate: Option<String>,
    focus_search: bool,
    focus_candidate: bool,
    scroll_candidate: bool,
    result_ids: Vec<(egui::Id, String)>,
    composing: bool,
    #[cfg(test)]
    searches: usize,
}

struct SearchPage {
    path: String,
    title: String,
    text: String,
    folded_path: String,
}

impl PageDirectory {
    pub(super) fn set_composing(&mut self, composing: bool) {
        self.composing = composing;
    }

    pub(super) fn show(&mut self) {
        self.visible = true;
        self.focus_search = true;
        self.scroll_candidate = true;
    }

    pub(super) fn sync(&mut self, preview: &ReaderExportPreview) {
        let current = self.bundle.as_ref().is_some_and(|(digest, baseline)| {
            digest == &preview.plan_digest && baseline == &preview.content_baseline
        });
        if !current {
            *self = Self::default();
            self.bundle = Some((
                preview.plan_digest.clone(),
                preview.content_baseline.clone(),
            ));
            self.index = preview
                .content
                .iter()
                .map(|page| SearchPage {
                    path: page.output_path.clone(),
                    title: page.title.to_lowercase(),
                    text: page.text.to_lowercase(),
                    folded_path: page.output_path.to_lowercase(),
                })
                .collect();
            self.opened_path = self.index.first().map(|page| page.path.clone());
        }
        self.search();
    }

    fn search(&mut self) -> bool {
        if self.cached_query.as_ref() == Some(&self.query) {
            return false;
        }
        let query = self.query.trim().to_lowercase();
        self.matches = self
            .index
            .iter()
            .enumerate()
            .filter_map(|(index, page)| {
                (page.title.contains(&query)
                    || page.text.contains(&query)
                    || page.folded_path.contains(&query))
                .then_some(index)
            })
            .collect();
        self.cached_query = Some(self.query.clone());
        self.page = 0;
        self.candidate = self
            .matches
            .first()
            .map(|index| self.index[*index].path.clone());
        self.result_ids.clear();
        self.scroll_candidate = true;
        #[cfg(test)]
        {
            self.searches += 1;
        }
        true
    }

    fn page_count(&self) -> usize {
        self.matches.len().div_ceil(PAGE_SIZE)
    }

    fn range(&self) -> std::ops::Range<usize> {
        let start = self.page * PAGE_SIZE;
        start..(start + PAGE_SIZE).min(self.matches.len())
    }

    fn set_page(&mut self, page: usize) {
        self.page = page.min(self.page_count().saturating_sub(1));
        self.candidate = self
            .matches
            .get(self.page * PAGE_SIZE)
            .map(|index| self.index[*index].path.clone());
        self.scroll_candidate = true;
    }

    fn move_candidate(&mut self, down: bool) {
        let current = self
            .matches
            .iter()
            .position(|index| self.candidate.as_ref() == Some(&self.index[*index].path))
            .unwrap_or(0);
        let next = if down {
            current
                .saturating_add(1)
                .min(self.matches.len().saturating_sub(1))
        } else {
            current.saturating_sub(1)
        };
        self.candidate = self
            .matches
            .get(next)
            .map(|index| self.index[*index].path.clone());
        self.page = next / PAGE_SIZE;
        self.scroll_candidate = true;
    }

    fn open(&mut self, path: &str) {
        if self.index.iter().any(|page| page.path == path) {
            self.opened_path = Some(path.to_owned());
            self.visible = false;
            self.result_ids.clear();
        }
    }

    pub(super) fn opened_index(&self) -> Option<usize> {
        self.index
            .iter()
            .position(|page| self.opened_path.as_ref() == Some(&page.path))
    }

    pub(super) fn open_in_public_order(&mut self, index: usize) {
        if let Some(page) = self.index.get(index) {
            self.opened_path = Some(page.path.clone());
        }
    }
}

#[cfg(test)]
mod tests;
