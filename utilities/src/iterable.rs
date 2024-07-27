use std::{collections::HashMap, hash::Hash};

pub fn index<K: Hash + Eq + Clone, V, I>(items: I) -> HashMap<K, Vec<V>>
    where I: IntoIterator<Item = (K, V)> {
    use itertools::Itertools;

    items
        .into_iter()
        .into_group_map_by(|(k, _)| k.clone())
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().map(|(_, vv)| vv).collect_vec()))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::iterable::index;

    #[test]
    fn test_index() {
        let items = vec![("a", 1), ("b", 2), ("a", 3)];
        let ix = index(items);
        assert_eq!(ix["a"], vec![1, 3]);
        assert_eq!(ix["b"], vec![2]);
    }
}