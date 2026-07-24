use proptest::prelude::*;

use super::super::{PlaceholderIter, PlaceholderPart};

proptest! {
    /// Verifies arbitrary UTF-8 input is partitioned losslessly without parser panics.
    #[test]
    fn placeholder_iteration_is_lossless(sql in any::<String>()) {
        let mut rebuilt = String::with_capacity(sql.len());
        for part in PlaceholderIter::new(&sql) {
            match part {
                PlaceholderPart::Sql(value) => rebuilt.push_str(value),
                PlaceholderPart::Placeholder(name) => {
                    rebuilt.push(':');
                    rebuilt.push_str(name);
                }
            }
        }
        prop_assert_eq!(rebuilt, sql);
    }
}
