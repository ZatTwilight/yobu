use crate::protocol::Entry;
use neo_frizbee::{Config, Matcher};
use std::collections::HashMap;

/// A menu catalog with precomputed strings for fuzzy matching.
pub struct SearchIndex {
    entries: Vec<Entry>,
    haystacks: Vec<String>,
}

impl SearchIndex {
    pub fn new(entries: Vec<Entry>) -> Self {
        let haystacks = entries
            .iter()
            .map(|entry| match entry.description.as_deref() {
                Some(description) if !description.is_empty() => {
                    format!("{} {description}", entry.label)
                }
                _ => entry.label.clone(),
            })
            .collect();

        Self { entries, haystacks }
    }

    /// Returns fuzzy matches ordered by match quality, then application recency.
    /// An empty query is ordered by recency and then catalog order.
    pub fn search(
        &self,
        query: &str,
        max_results: usize,
        recent_applications: &[String],
    ) -> Vec<Entry> {
        if max_results == 0 {
            return Vec::new();
        }

        let recency: HashMap<&str, usize> = recent_applications
            .iter()
            .enumerate()
            .map(|(rank, id)| (id.as_str(), rank))
            .collect();
        let recency_rank = |index: u32| {
            recency
                .get(self.entries[index as usize].id.as_str())
                .copied()
                .unwrap_or(usize::MAX)
        };

        let query = query.trim();
        let mut matches = if query.is_empty() {
            (0..self.entries.len())
                .map(neo_frizbee::Match::from_index)
                .collect()
        } else {
            Matcher::new(query, &Config::default()).match_list(&self.haystacks)
        };

        matches.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| recency_rank(left.index).cmp(&recency_rank(right.index)))
                .then_with(|| left.index.cmp(&right.index))
        });

        matches
            .into_iter()
            .take(max_results)
            .map(|matched| self.entries[matched.index as usize].clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, label: &str, description: Option<&str>) -> Entry {
        Entry {
            id: id.into(),
            label: label.into(),
            description: description.map(str::to_owned),
            icon: None,
            value: id.into(),
        }
    }

    #[test]
    fn fuzzy_matches_labels_and_descriptions() {
        let index = SearchIndex::new(vec![
            entry("terminal", "Terminal Emulator", None),
            entry("browser", "Web Browser", Some("Browse the internet")),
            entry("files", "Files", None),
        ]);

        assert_eq!(index.search("trmnl", 10, &[])[0].id, "terminal");
        assert_eq!(index.search("internet", 10, &[])[0].id, "browser");
    }

    #[test]
    fn recency_orders_empty_queries_and_equal_matches() {
        let index = SearchIndex::new(vec![
            entry("first", "Same", None),
            entry("recent", "Same", None),
            entry("other", "Other", None),
        ]);
        let recent = vec!["recent".into()];

        let empty: Vec<_> = index
            .search("", 3, &recent)
            .into_iter()
            .map(|entry| entry.id)
            .collect();
        assert_eq!(empty, ["recent", "first", "other"]);

        let matched: Vec<_> = index
            .search("same", 2, &recent)
            .into_iter()
            .map(|entry| entry.id)
            .collect();
        assert_eq!(matched, ["recent", "first"]);
    }

    #[test]
    fn fuzzy_score_takes_priority_over_recency() {
        let index = SearchIndex::new(vec![
            entry("exact", "alpha", None),
            entry("recent", "alphabet soup", None),
        ]);

        assert_eq!(index.search("alpha", 2, &["recent".into()])[0].id, "exact");
    }

    #[test]
    fn limits_results() {
        let index = SearchIndex::new(vec![entry("one", "One", None), entry("two", "Two", None)]);

        assert_eq!(index.search("", 1, &[]).len(), 1);
        assert!(index.search("", 0, &[]).is_empty());
    }
}
