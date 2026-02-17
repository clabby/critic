use crate::domain::{CommentRef, ListNode};
use std::collections::HashSet;

pub(super) fn filter_thread_nodes(nodes: &[ListNode], query: &str) -> Vec<ListNode> {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return nodes.to_vec();
    }

    let mut include = HashSet::<String>::new();
    let mut parent_stack = Vec::<String>::new();

    for node in nodes {
        while parent_stack.len() > node.depth {
            parent_stack.pop();
        }

        if node_matches_query(node, &query) {
            include.insert(node.key.clone());
            for parent_key in &parent_stack {
                include.insert(parent_key.clone());
            }
        }

        parent_stack.push(node.key.clone());
    }

    nodes
        .iter()
        .filter(|node| include.contains(&node.key))
        .cloned()
        .collect()
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
