//! 候选是查找入口，原profile才是已有授权；无效项和未知能力不得悄悄丢弃。
use super::*;
use worldline_core::reader_export::*;

fn ordered<T: Clone + Ord>(selected: &BTreeSet<T>, previous: &[T], candidates: &[T]) -> Vec<T> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for value in previous {
        if selected.contains(value) {
            // 连原配置中的重复项也原样保留，交给core严格拒绝，不静默修正授权。
            result.push(value.clone());
            seen.insert(value);
        }
    }
    for value in candidates.iter().chain(selected.iter()) {
        if selected.contains(value) && seen.insert(value) {
            result.push(value.clone());
        }
    }
    result
}

impl ReaderPublishState {
    pub(super) fn selection(&self) -> ReaderExportSelection {
        let previous = self.profile.as_ref().map(|profile| &profile.selection);
        let schema_version = previous.map_or(READER_SITE_SCHEMA_VERSION, |selection| {
            selection.schema_version
        });
        let mut features = previous.map_or_else(
            || vec![READER_SITE_FEATURE.into()],
            |selection| selection.required_features.clone(),
        );
        let had_story = previous.is_some_and(|selection| {
            selection
                .required_features
                .iter()
                .any(|feature| feature == READER_STORY_FEATURE)
        });
        if had_story != self.story_details {
            features.retain(|feature| feature != READER_STORY_FEATURE);
            if self.story_details {
                features.push(READER_STORY_FEATURE.into());
            }
        }
        let field_targets = self
            .fields
            .iter()
            .filter(|(_, keys)| !keys.is_empty())
            .map(|(target, _)| target.clone())
            .collect();
        let previous_fields = previous
            .map(|selection| selection.fields.as_slice())
            .unwrap_or_default();
        let mut previous_field_keys = BTreeMap::new();
        for field in previous_fields {
            previous_field_keys
                .entry(&field.target)
                .or_insert(field.keys.as_slice());
        }
        let fields: Vec<_> = ordered(
            &field_targets,
            &previous_fields
                .iter()
                .map(|field| field.target.clone())
                .collect::<Vec<_>>(),
            &[],
        )
        .into_iter()
        .map(|target| {
            let previous_keys = previous_field_keys
                .get(&target)
                .copied()
                .unwrap_or_default();
            ReaderFieldSelection {
                keys: ordered(&self.fields[&target], previous_keys, &[]),
                target,
            }
        })
        .collect();
        if !fields.is_empty()
            && previous.is_none_or(|selection| selection.fields != fields)
            && !features
                .iter()
                .any(|feature| feature == READER_FIELDS_FEATURE)
        {
            features.push(READER_FIELDS_FEATURE.into());
        }
        let book_ids = self
            .chapters
            .iter()
            .filter(|(_, chapters)| !chapters.is_empty())
            .map(|(id, _)| id.clone())
            .collect();
        let previous_books = previous
            .map(|selection| selection.manuscripts.as_slice())
            .unwrap_or_default();
        let manuscripts = ordered(
            &book_ids,
            &previous_books
                .iter()
                .map(|book| book.id.clone())
                .collect::<Vec<_>>(),
            &self
                .manuscript_choices
                .iter()
                .map(|book| book.id.clone())
                .collect::<Vec<_>>(),
        )
        .into_iter()
        .map(|id| {
            let old = previous_books
                .iter()
                .find(|book| book.id == id)
                .map(|book| book.chapters.as_slice())
                .unwrap_or_default();
            let candidates = self
                .manuscript_choices
                .iter()
                .find(|book| book.id == id)
                .map(|book| {
                    book.chapters
                        .iter()
                        .map(|(id, _)| id.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            ReaderManuscriptSelection {
                chapters: ordered(&self.chapters[&id], old, &candidates),
                id,
            }
        })
        .collect();
        let map_ids = self.maps.keys().cloned().collect();
        let maps = ordered(
            &map_ids,
            &previous
                .map(|selection| {
                    selection
                        .maps
                        .iter()
                        .map(|map| map.id.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            &self
                .map_choices
                .iter()
                .map(|map| map.id.clone())
                .collect::<Vec<_>>(),
        )
        .into_iter()
        .map(|id| self.maps[&id].clone())
        .collect();
        ReaderExportSelection {
            schema_version,
            required_features: features,
            site_title: self.site_title.clone(),
            objects: ordered(
                &self.objects,
                previous
                    .map(|selection| selection.objects.as_slice())
                    .unwrap_or_default(),
                &self
                    .object_choices
                    .iter()
                    .map(|choice| choice.target.clone())
                    .collect::<Vec<_>>(),
            ),
            fields,
            manuscripts,
            maps,
            attachments: ordered(
                &self.attachments,
                previous
                    .map(|selection| selection.attachments.as_slice())
                    .unwrap_or_default(),
                &self
                    .attachment_choices
                    .iter()
                    .map(|choice| choice.id.clone())
                    .collect::<Vec<_>>(),
            ),
        }
    }

    pub(super) fn current_profile(&self) -> Option<ReaderPublicationProfile> {
        self.profile.clone().map(|mut profile| {
            profile.selection = self.selection();
            profile.title = self.profile_title.clone();
            profile
        })
    }
}

#[cfg(test)]
mod tests;
