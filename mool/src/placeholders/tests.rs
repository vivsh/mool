use super::*;

pub(super) fn collect_parts(sql: &str) -> Vec<PlaceholderPart<'_>> {
    PlaceholderIter::new(sql).collect()
}

pub(super) fn parts_to_strings(parts: Vec<PlaceholderPart>) -> Vec<String> {
    parts
        .into_iter()
        .map(|p| match p {
            PlaceholderPart::Sql(s) => format!("SQL:{}", s),
            PlaceholderPart::Placeholder(n) => format!("PARAM:{}", n),
        })
        .collect()
}

mod iter;
mod resolve;
