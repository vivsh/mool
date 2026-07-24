use std::collections::BTreeSet;

use mool as db;
use mool::Model;
use proptest::prelude::*;

#[derive(Debug, Clone, db::Model)]
#[table(name = "property_rows")]
struct PropertyRow {
    id: i64,
    value: i64,
}

proptest! {
    /// Verifies generated membership plans expose a complete contiguous bind-position set.
    #[test]
    fn membership_plan_metadata_matches_input_width(values in prop::collection::vec(any::<i64>(), 0..64)) {
        let rows = PropertyRow::table();
        let expected = values.len();
        let plan = db::from(&rows)
            .filter(rows.id.in_values(values))
            .all::<PropertyRow>()
            .plan()
            .expect("generated membership plan");

        prop_assert_eq!(plan.prebound_count, 0);
        prop_assert_eq!(plan.dynamic_bind_count, expected);
        prop_assert_eq!(plan.total_bind_count, expected);
        prop_assert_eq!(planned_positions(&plan), expected_positions(expected));
    }

    /// Verifies arbitrary arithmetic expression depth preserves bind metadata invariants.
    #[test]
    fn arithmetic_tree_metadata_matches_leaf_count(values in prop::collection::vec(any::<i64>(), 0..48)) {
        let rows = PropertyRow::table();
        let mut expression = db::val(0_i64);
        for value in &values {
            expression = expression.plus(db::val(*value));
        }
        let plan = db::from(&rows)
            .scalar(expression)
            .plan()
            .expect("generated arithmetic plan");

        let expected = values.len() + 1;
        prop_assert_eq!(plan.dynamic_bind_count, expected);
        prop_assert_eq!(plan.total_bind_count, expected);
        prop_assert_eq!(planned_positions(&plan), expected_positions(expected));
    }
}

fn planned_positions(plan: &db::QueryPlan) -> BTreeSet<usize> {
    plan.params
        .values()
        .flat_map(|parameter| parameter.occurrences.iter().copied())
        .collect()
}

fn expected_positions(count: usize) -> BTreeSet<usize> {
    (1..=count).collect()
}
