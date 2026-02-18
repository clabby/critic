//! Fuzzy matching helpers for pull request search.

use crate::domain::PullRequestSummary;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};

/// A ranked fuzzy search result.
#[derive(Debug, Clone, Copy)]
pub struct FuzzyResult {
    pub index: usize,
    pub score: i64,
}

/// Ranks pull requests using `fuzzy-matcher` (Skim algorithm).
pub fn rank_pull_requests(query: &str, pulls: &[PullRequestSummary]) -> Vec<FuzzyResult> {
    let trimmed = query.trim();

    if trimmed.is_empty() {
        return pulls
            .iter()
            .enumerate()
            .map(|(index, pull)| FuzzyResult {
                index,
                score: pull.updated_at_unix_ms,
            })
            .collect();
    }

    let matcher = SkimMatcherV2::default().smart_case();

    let mut results: Vec<FuzzyResult> = pulls
        .iter()
        .enumerate()
        .filter_map(|(index, pull)| {
            matcher
                .fuzzy_match(&pull.search_text(), trimmed)
                .map(|score| FuzzyResult { index, score })
        })
        .collect();

    results.sort_by_key(|result| std::cmp::Reverse(result.score));
    results
}

#[cfg(test)]
mod tests {
    use super::rank_pull_requests;
    use crate::domain::{PullRequestReviewStatus, PullRequestSummary};

    fn pull(number: u64, title: &str, author: &str) -> PullRequestSummary {
        PullRequestSummary {
            owner: "acme".to_owned(),
            repo: "widget".to_owned(),
            number,
            title: title.to_owned(),
            author: author.to_owned(),
            head_ref: format!("feature/{number}"),
            base_ref: "main".to_owned(),
            head_sha: format!("{number:040x}"),
            base_sha: format!("{:040x}", number + 1),
            html_url: None,
            updated_at_unix_ms: number as i64,
            created_at_unix_ms: number as i64,
            review_status: Some(PullRequestReviewStatus::Approved),
        }
    }

    #[test]
    fn empty_query_returns_all() {
        let pulls = vec![pull(1, "alpha", "alice"), pull(2, "beta", "bob")];
        let ranked = rank_pull_requests("", &pulls);
        assert_eq!(ranked.len(), 2);
    }

    #[test]
    fn query_filters_and_scores() {
        let pulls = vec![
            pull(1, "fix networking", "alice"),
            pull(2, "add parser", "bob"),
        ];
        let ranked = rank_pull_requests("network", &pulls);

        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].index, 0);
    }
}
