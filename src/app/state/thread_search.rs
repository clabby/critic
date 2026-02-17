use super::tree_filter::filter_with_ancestors;
use crate::domain::{CommentRef, ListNode};

pub(super) fn filter_thread_nodes(nodes: &[ListNode], query: &str) -> Vec<ListNode> {
    filter_with_ancestors(
        nodes,
        query,
        |node| node.key.as_str(),
        |node| node.depth,
        node_matches_query,
    )
}

fn node_matches_query(node: &ListNode, query: &str) -> bool {
    contains_ignore_case(node.comment.author(), query)
        || contains_ignore_case(node.comment.body(), query)
        || match &node.comment {
            CommentRef::Review(comment) => contains_ignore_case(comment.path.as_str(), query),
            CommentRef::Issue(_) => false,
            CommentRef::ReviewSummary(_) => false,
        }
}

fn contains_ignore_case(value: &str, query: &str) -> bool {
    value.to_ascii_lowercase().contains(query)
}
