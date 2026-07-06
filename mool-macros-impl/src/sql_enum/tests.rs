use super::model::ParsedSqlEnum;
use syn::parse_quote;

/// Verifies default SQL enum labels use snake_case.
#[test]
fn default_labels_are_snake_case() {
    let input = parse_quote! {
        enum PostStatus {
            Draft,
            InReview,
        }
    };
    let parsed = ParsedSqlEnum::from_input(&input).expect("enum should parse");
    assert_eq!(parsed.sql_name, "post_status");
    assert_eq!(parsed.variants[0].label, "draft");
    assert_eq!(parsed.variants[1].label, "in_review");
}

/// Verifies integer storage requires explicit codes.
#[test]
fn int_storage_requires_codes() {
    let input = parse_quote! {
        #[sql_enum(storage = "int")]
        enum PostStatus {
            Draft,
        }
    };
    let err = ParsedSqlEnum::from_input(&input).expect_err("missing code should fail");
    assert!(err.to_string().contains("requires every variant"));
}

/// Verifies duplicate labels are rejected.
#[test]
fn duplicate_labels_are_rejected() {
    let input = parse_quote! {
        enum PostStatus {
            #[sql_enum(value = "same")]
            Draft,
            #[sql_enum(value = "same")]
            Published,
        }
    };
    let err = ParsedSqlEnum::from_input(&input).expect_err("duplicate label should fail");
    assert!(err.to_string().contains("duplicate SQL enum label"));
}
