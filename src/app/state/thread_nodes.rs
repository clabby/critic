use crate::domain::{CommentRef, ListNode, ListNodeKind, ReviewThread, review_comment_is_outdated};
use std::collections::{HashMap, HashSet};

fn append_reply_nodes(
    nodes: &mut Vec<ListNode>,
    thread: &ReviewThread,
    depth: usize,
    root_key: &str,
) {
    for reply in &thread.replies {
        nodes.push(ListNode {
            key: format!("reply:{}", reply.comment.id.into_inner()),
            kind: ListNodeKind::Reply,
            depth,
            root_key: Some(root_key.to_owned()),
            is_resolved: thread.is_resolved,
            is_outdated: review_comment_is_outdated(&reply.comment),
            comment: CommentRef::Review(reply.comment.clone()),
        });

        append_reply_nodes(nodes, reply, depth + 1, root_key);
    }
}

pub(super) fn append_thread_nodes(
    nodes: &mut Vec<ListNode>,
    threads_by_key: &mut HashMap<String, ReviewThread>,
    collapsed: &HashSet<String>,
    thread: &ReviewThread,
    depth: usize,
) {
    let key = thread_key(thread);
    threads_by_key.insert(key.clone(), thread.clone());

    nodes.push(ListNode {
        key: key.clone(),
        kind: ListNodeKind::Thread,
        depth,
        root_key: Some(key.clone()),
        is_resolved: thread.is_resolved,
        is_outdated: review_comment_is_outdated(&thread.comment),
        comment: CommentRef::Review(thread.comment.clone()),
    });

    if collapsed.contains(&key) {
        return;
    }

    append_reply_nodes(nodes, thread, depth + 1, &key);
}

pub(super) fn thread_key(thread: &ReviewThread) -> String {
    match thread
        .thread_id
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        Some(id) => format!("thread:{id}"),
        None => format!("comment:{}", thread.comment.id.into_inner()),
    }
}

pub(super) fn review_group_key(review_id: u64) -> String {
    format!("review-group:{review_id}")
}

pub(super) fn is_review_group_key(key: &str) -> bool {
    key.starts_with("review-group:")
}
