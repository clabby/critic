use std::collections::HashSet;

pub(super) fn filter_with_ancestors<T, FKey, FDepth, FMatch>(
    items: &[T],
    query: &str,
    key_of: FKey,
    depth_of: FDepth,
    matches: FMatch,
) -> Vec<T>
where
    T: Clone,
    FKey: Fn(&T) -> &str,
    FDepth: Fn(&T) -> usize,
    FMatch: Fn(&T, &str) -> bool,
{
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return items.to_vec();
    }

    let mut include = HashSet::<String>::new();
    let mut parent_stack = Vec::<String>::new();

    for item in items {
        while parent_stack.len() > depth_of(item) {
            parent_stack.pop();
        }

        if matches(item, &query) {
            include.insert(key_of(item).to_owned());
            for parent_key in &parent_stack {
                include.insert(parent_key.clone());
            }
        }

        parent_stack.push(key_of(item).to_owned());
    }

    items
        .iter()
        .filter(|item| include.contains(key_of(item)))
        .cloned()
        .collect()
}
