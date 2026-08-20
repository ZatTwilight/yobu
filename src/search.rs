use crate::{config::SavedState, protocol::Entry};
use neo_frizbee::{Config, Matcher};
use std::cmp::Ordering;

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

    /// Returns fuzzy matches ordered by match quality, then frecency (frequency × recency decay).
    /// An empty query is ordered purely by frecency then catalog order.
    /// Frecency uses half-life 14 days, matching fff's frecency idea.
    pub fn search(&self, query: &str, max_results: usize, state: &SavedState) -> Vec<Entry> {
        if max_results == 0 {
            return Vec::new();
        }

        let now = crate::config::now_unix();
        let frecency = |index: u32| state.frecency_score(&self.entries[index as usize].id, now);

        let query = query.trim();
        let mut matches = if query.is_empty() {
            (0..self.entries.len())
                .map(neo_frizbee::Match::from_index)
                .collect()
        } else {
            Matcher::new(query, &Config::default()).match_list(&self.haystacks)
        };

        matches.sort_by(|left, right| {
            // For empty queries all scores are 0 — frecency dominates.
            // For non-empty, fuzzy score dominates, frecency breaks ties.
            let score_ord = right.score.cmp(&left.score);
            if score_ord != Ordering::Equal {
                return score_ord;
            }
            let left_f = frecency(left.index);
            let right_f = frecency(right.index);
            // Sort descending by frecency
            match right_f.partial_cmp(&left_f).unwrap_or(Ordering::Equal) {
                Ordering::Equal => left.index.cmp(&right.index),
                ord => ord,
            }
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
    use crate::config::{FrecencyEntry, SavedState};

    fn entry(id: &str, label: &str, description: Option<&str>) -> Entry {
        Entry {
            id: id.into(),
            label: label.into(),
            description: description.map(str::to_owned),
            icon: None,
            preview: None,
            value: id.into(),
        }
    }

    fn state_with_frecency(ids: &[&str]) -> SavedState {
        // Build frecency where first id is most recent/frequent
        let mut state = SavedState::default();
        let now = crate::config::now_unix();
        for (rank, id) in ids.iter().enumerate() {
            state.frecency.insert(
                (*id).to_owned(),
                FrecencyEntry {
                    count: (ids.len() - rank) as u32,
                    last_access: now - (rank as u64) * 60,
                },
            );
        }
        state
    }

    #[test]
    fn fuzzy_matches_labels_and_descriptions() {
        let index = SearchIndex::new(vec![
            entry("terminal", "Terminal Emulator", None),
            entry("browser", "Web Browser", Some("Browse the internet")),
            entry("files", "Files", None),
        ]);

        assert_eq!(
            index.search("trmnl", 10, &SavedState::default())[0].id,
            "terminal"
        );
        assert_eq!(
            index.search("internet", 10, &SavedState::default())[0].id,
            "browser"
        );
    }

    #[test]
    fn frecency_orders_empty_queries_and_equal_matches() {
        let index = SearchIndex::new(vec![
            entry("first", "Same", None),
            entry("recent", "Same", None),
            entry("other", "Other", None),
        ]);
        let state = state_with_frecency(&["recent"]);

        let empty: Vec<_> = index
            .search("", 3, &state)
            .into_iter()
            .map(|entry| entry.id)
            .collect();
        assert_eq!(empty, ["recent", "first", "other"]);

        let matched: Vec<_> = index
            .search("same", 2, &state)
            .into_iter()
            .map(|entry| entry.id)
            .collect();
        assert_eq!(matched, ["recent", "first"]);
    }

    #[test]
    fn frequency_beats_recency() {
        // frequent but slightly older should beat single recent
        let index = SearchIndex::new(vec![
            entry("recent", "Same", None),
            entry("frequent", "Same", None),
        ]);
        let mut state = SavedState::default();
        let now = crate::config::now_unix();
        state.frecency.insert(
            "recent".into(),
            FrecencyEntry {
                count: 1,
                last_access: now,
            },
        );
        state.frecency.insert(
            "frequent".into(),
            FrecencyEntry {
                count: 5,
                last_access: now - 3600,
            },
        );
        let ordered: Vec<_> = index
            .search("same", 2, &state)
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(ordered[0], "frequent");
    }

    #[test]
    fn fuzzy_score_takes_priority_over_frecency() {
        let index = SearchIndex::new(vec![
            entry("exact", "alpha", None),
            entry("recent", "alphabet soup", None),
        ]);
        let state = state_with_frecency(&["recent"]);
        assert_eq!(index.search("alpha", 2, &state)[0].id, "exact");
    }

    #[test]
    fn limits_results() {
        let index = SearchIndex::new(vec![entry("one", "One", None), entry("two", "Two", None)]);

        assert_eq!(index.search("", 1, &SavedState::default()).len(), 1);
        assert!(index.search("", 0, &SavedState::default()).is_empty());
    }
}
