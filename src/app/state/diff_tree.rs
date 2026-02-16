use super::DiffTreeRow;
use crate::domain::PullRequestDiffData;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Default, Clone)]
struct DiffTreeNode {
    children: BTreeMap<String, DiffTreeNode>,
    files: Vec<usize>,
}

pub(super) fn build_diff_tree_rows(
    diff: &PullRequestDiffData,
    collapsed: &HashSet<String>,
) -> Vec<DiffTreeRow> {
    let mut root = DiffTreeNode::default();

    for (index, file) in diff.files.iter().enumerate() {
        insert_diff_path(&mut root, &file.path, index);
    }
    sort_diff_tree_files(&mut root, diff);

    let mut rows = Vec::new();
    append_diff_tree_rows(&root, "", 0, &mut rows, diff, collapsed);
    rows
}

pub(super) fn filter_diff_tree_rows(
    rows: &[DiffTreeRow],
    diff: &PullRequestDiffData,
    query: &str,
) -> Vec<DiffTreeRow> {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return rows.to_vec();
    }

    let mut include = HashSet::<String>::new();
    let mut parent_stack = Vec::<String>::new();

    for row in rows {
        while parent_stack.len() > row.depth {
            parent_stack.pop();
        }

        if row.is_directory {
            parent_stack.push(row.key.clone());
            continue;
        }

        let matches = row
            .file_index
            .and_then(|file_index| diff.files.get(file_index))
            .is_some_and(|file| file.path.to_ascii_lowercase().contains(&query));

        if matches {
            include.insert(row.key.clone());
            for key in &parent_stack {
                include.insert(key.clone());
            }
        }
    }

    rows.iter()
        .filter(|row| include.contains(&row.key))
        .cloned()
        .map(|mut row| {
            row.is_collapsed = false;
            row
        })
        .collect()
}

fn insert_diff_path(root: &mut DiffTreeNode, path: &str, file_index: usize) {
    let parts = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        root.files.push(file_index);
        return;
    }

    if parts.len() == 1 {
        root.files.push(file_index);
        return;
    }

    let mut node = root;
    for segment in &parts[..parts.len() - 1] {
        node = node.children.entry((*segment).to_owned()).or_default();
    }
    node.files.push(file_index);
}

fn sort_diff_tree_files(node: &mut DiffTreeNode, diff: &PullRequestDiffData) {
    node.files
        .sort_by(|a, b| diff.files[*a].path.cmp(&diff.files[*b].path));
    for child in node.children.values_mut() {
        sort_diff_tree_files(child, diff);
    }
}

fn append_diff_tree_rows(
    node: &DiffTreeNode,
    parent_key: &str,
    depth: usize,
    rows: &mut Vec<DiffTreeRow>,
    diff: &PullRequestDiffData,
    collapsed: &HashSet<String>,
) {
    for (segment, child) in &node.children {
        let (dir_label, dir_key, compressed) = compress_directory(parent_key, segment, child);
        let is_collapsed = collapsed.contains(&dir_key);

        rows.push(DiffTreeRow {
            key: dir_key.clone(),
            label: dir_label,
            depth,
            is_directory: true,
            is_collapsed,
            file_index: None,
        });

        if !is_collapsed {
            append_diff_tree_rows(compressed, &dir_key, depth + 1, rows, diff, collapsed);
        }
    }

    for file_index in &node.files {
        let file = &diff.files[*file_index];
        let label = file
            .path
            .rsplit('/')
            .next()
            .unwrap_or(file.path.as_str())
            .to_owned();
        rows.push(DiffTreeRow {
            key: format!("file:{}", file.path),
            label,
            depth,
            is_directory: false,
            is_collapsed: false,
            file_index: Some(*file_index),
        });
    }
}

fn compress_directory<'a>(
    parent_key: &str,
    initial_segment: &str,
    initial_node: &'a DiffTreeNode,
) -> (String, String, &'a DiffTreeNode) {
    let mut label = initial_segment.to_owned();
    let mut key = join_path(parent_key, initial_segment);
    let mut node = initial_node;

    while node.files.is_empty() && node.children.len() == 1 {
        let Some((segment, next)) = node.children.iter().next() else {
            break;
        };
        label.push('/');
        label.push_str(segment);
        key.push('/');
        key.push_str(segment);
        node = next;
    }

    (label, key, node)
}

fn join_path(parent: &str, segment: &str) -> String {
    if parent.is_empty() {
        segment.to_owned()
    } else {
        format!("{parent}/{segment}")
    }
}
