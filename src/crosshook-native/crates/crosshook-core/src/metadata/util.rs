/// Build a comma-joined list of `?` placeholders for SQL `IN (...)` clauses.
pub(super) fn in_clause_placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(", ")
}
