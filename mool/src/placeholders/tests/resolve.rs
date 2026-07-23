use super::*;

/// Verifies placeholder parsing and resolution for `resolve postgres placeholders`.
#[test]
fn resolve_postgres_placeholders() {
    use std::collections::HashMap;
    let sql = "SELECT * FROM users WHERE id = :id AND name = :name";
    let mut args = crate::commons::Arguments::default();
    let mut values = HashMap::new();
    values.insert("id".to_string(), crate::argvalue::ArgValue::new(42i32));
    values.insert(
        "name".to_string(),
        crate::argvalue::ArgValue::new("Alice".to_string()),
    );

    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Postgres);
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap(),
        "SELECT * FROM users WHERE id = $1 AND name = $2"
    );
}

/// Verifies placeholder parsing and resolution for `resolve mysql placeholders`.
#[test]
fn resolve_mysql_placeholders() {
    use std::collections::HashMap;
    let sql = "SELECT * FROM users WHERE id = :id AND name = :name";
    let mut args = crate::commons::Arguments::default();
    let mut values = HashMap::new();
    values.insert("id".to_string(), crate::argvalue::ArgValue::new(42i32));
    values.insert(
        "name".to_string(),
        crate::argvalue::ArgValue::new("Alice".to_string()),
    );

    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Mysql);
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap(),
        "SELECT * FROM users WHERE id = ? AND name = ?"
    );
}

/// Verifies placeholder parsing and resolution for `resolve sqlite placeholders`.
#[test]
fn resolve_sqlite_placeholders() {
    use std::collections::HashMap;
    let sql = "SELECT * FROM users WHERE id = :id";
    let mut args = crate::commons::Arguments::default();
    let mut values = HashMap::new();
    values.insert("id".to_string(), crate::argvalue::ArgValue::new(42i32));

    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Sqlite);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "SELECT * FROM users WHERE id = ?");
}

/// Verifies placeholder parsing and resolution for `resolve missing placeholder`.
#[test]
fn resolve_missing_placeholder() {
    use std::collections::HashMap;
    let sql = "SELECT * FROM users WHERE id = :id AND name = :name";
    let mut args = crate::commons::Arguments::default();
    let values = HashMap::new(); // empty map

    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Postgres);
    assert!(result.is_err());
    match result.unwrap_err() {
        PlaceholderError::MissingValue(name) => {
            assert_eq!(name, "id");
        }
        _ => panic!("Expected MissingValue error"),
    }
}

/// Verifies placeholder parsing and resolution for `resolve skips placeholders in quotes`.
#[test]
fn resolve_skips_placeholders_in_quotes() {
    use std::collections::HashMap;
    let sql = "SELECT ':fake' FROM users WHERE id = :id";
    let mut args = crate::commons::Arguments::default();
    let mut values = HashMap::new();
    values.insert("id".to_string(), crate::argvalue::ArgValue::new(42i32));

    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Postgres);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "SELECT ':fake' FROM users WHERE id = $1");
}

/// Verifies placeholder parsing and resolution for `resolve skips placeholders in comments`.
#[test]
fn resolve_skips_placeholders_in_comments() {
    use std::collections::HashMap;
    let sql = "SELECT * FROM users WHERE id = :id -- :fake";
    let mut args = crate::commons::Arguments::default();
    let mut values = HashMap::new();
    values.insert("id".to_string(), crate::argvalue::ArgValue::new(42i32));

    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Postgres);
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap(),
        "SELECT * FROM users WHERE id = $1 -- :fake"
    );
}

/// Verifies placeholder parsing and resolution for `resolve consecutive placeholders`.
#[test]
fn resolve_consecutive_placeholders() {
    use std::collections::HashMap;
    let sql = ":a:b:c";
    let mut args = crate::commons::Arguments::default();
    let mut values = HashMap::new();
    values.insert("a".to_string(), crate::argvalue::ArgValue::new(1i32));
    values.insert("b".to_string(), crate::argvalue::ArgValue::new(2i32));
    values.insert("c".to_string(), crate::argvalue::ArgValue::new(3i32));

    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Postgres);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "$1$2$3");
}

/// Verifies placeholder parsing and resolution for `resolve empty sql`.
#[test]
fn resolve_empty_sql() {
    use std::collections::HashMap;
    let sql = "";
    let mut args = crate::commons::Arguments::default();
    let values = HashMap::new();

    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Postgres);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

/// Verifies placeholder parsing and resolution for `resolve increments arguments len`.
#[test]
fn resolve_increments_arguments_len() {
    use sqlx::Arguments as _;
    use std::collections::HashMap;
    let sql = "SELECT * FROM users WHERE id = :id AND name = :name AND age = :age";
    let mut args = crate::commons::Arguments::default();
    let mut values = HashMap::new();
    values.insert("id".to_string(), crate::argvalue::ArgValue::new(42i32));
    values.insert(
        "name".to_string(),
        crate::argvalue::ArgValue::new("Bob".to_string()),
    );
    values.insert("age".to_string(), crate::argvalue::ArgValue::new(30i32));

    assert_eq!(args.len(), 0);

    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Postgres);
    assert!(result.is_ok());
    assert_eq!(args.len(), 3);
    assert_eq!(
        result.unwrap(),
        "SELECT * FROM users WHERE id = $1 AND name = $2 AND age = $3"
    );
}

/// Verifies placeholder parsing and resolution for `resolve continues from existing arguments`.
#[test]
fn resolve_continues_from_existing_arguments() {
    use sqlx::Arguments as _;
    use std::collections::HashMap;

    let mut args = crate::commons::Arguments::default();
    // Pre-bind some arguments
    args.add(&100i32).unwrap();
    args.add(&"existing").unwrap();
    assert_eq!(args.len(), 2);

    let sql = "WHERE status = :status AND type = :type";
    let mut values = HashMap::new();
    values.insert(
        "status".to_string(),
        crate::argvalue::ArgValue::new("active".to_string()),
    );
    values.insert("type".to_string(), crate::argvalue::ArgValue::new(5i32));

    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Postgres);
    assert!(result.is_ok());
    assert_eq!(args.len(), 4);
    assert_eq!(result.unwrap(), "WHERE status = $3 AND type = $4");
}

/// Verifies placeholder parsing and resolution for `resolve error on multiple missing values`.
#[test]
fn resolve_error_on_multiple_missing_values() {
    use std::collections::HashMap;
    let sql = "SELECT * FROM users WHERE id = :id AND name = :name AND email = :email";
    let mut args = crate::commons::Arguments::default();
    let mut values = HashMap::new();
    values.insert("id".to_string(), crate::argvalue::ArgValue::new(42i32));
    // name and email are missing

    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Postgres);
    assert!(result.is_err());
    match result.unwrap_err() {
        PlaceholderError::MissingValue(name) => {
            assert_eq!(name, "name"); // First missing placeholder
        }
        _ => panic!("Expected MissingValue error"),
    }
}

/// Verifies placeholder parsing and resolution for `resolve error preserves arguments on failure`.
#[test]
fn resolve_error_preserves_arguments_on_failure() {
    use sqlx::Arguments as _;
    use std::collections::HashMap;
    let sql = "SELECT * FROM users WHERE id = :id AND name = :missing";
    let mut args = crate::commons::Arguments::default();
    let mut values = HashMap::new();
    values.insert("id".to_string(), crate::argvalue::ArgValue::new(42i32));

    let initial_len = args.len();
    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Postgres);
    assert!(result.is_err());
    // Arguments should have been modified up to the point of error
    assert!(args.len() > initial_len);
    assert_eq!(args.len(), 1); // Only :id was bound before error
}

/// Verifies placeholder parsing and resolution for `resolve reuses postgres positions`.
#[test]
fn resolve_reuses_postgres_positions() {
    use sqlx::Arguments as _;
    use std::collections::HashMap;
    let sql = "SELECT * FROM users WHERE id = :id AND parent_id = :id AND status = :status";
    let mut args = crate::commons::Arguments::default();
    let mut values = HashMap::new();
    values.insert("id".to_string(), crate::argvalue::ArgValue::new(42i32));
    values.insert(
        "status".to_string(),
        crate::argvalue::ArgValue::new("active".to_string()),
    );

    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Postgres);
    assert!(result.is_ok());
    // :id should reuse $1, :status should be $2
    assert_eq!(
        result.unwrap(),
        "SELECT * FROM users WHERE id = $1 AND parent_id = $1 AND status = $2"
    );
    // Only 2 values should be bound even though there are 3 placeholders
    assert_eq!(args.len(), 2);
}

/// Verifies placeholder parsing and resolution for `resolve mysql with duplicate placeholders`.
#[test]
fn resolve_mysql_with_duplicate_placeholders() {
    use sqlx::Arguments as _;
    use std::collections::HashMap;
    let sql = "SELECT * FROM users WHERE id = :id OR parent_id = :id";
    let mut args = crate::commons::Arguments::default();
    let mut values = HashMap::new();
    values.insert("id".to_string(), crate::argvalue::ArgValue::new(42i32));

    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Mysql);
    assert!(result.is_ok());
    // MySQL uses ? for all, but only binds once
    assert_eq!(
        result.unwrap(),
        "SELECT * FROM users WHERE id = ? OR parent_id = ?"
    );
    assert_eq!(args.len(), 2); // Always binds for each placeholder in MySQL
}

// Regression coverage for UTF-8 around placeholders, quotes, and comments.

/// Verifies placeholder parsing and resolution for `iter with multibyte utf8 in sql`.
#[test]
fn iter_with_multibyte_utf8_in_sql() {
    // Multi-byte UTF-8 characters (emoji, accented chars, etc.)
    let sql = "SELECT * FROM café WHERE name = :name";
    let parts: Vec<_> = PlaceholderIter::new(sql).collect();
    assert_eq!(parts.len(), 2); // SQL before (with é), placeholder
    assert!(matches!(parts[0], PlaceholderPart::Sql(_)));
    assert_eq!(parts[1], PlaceholderPart::Placeholder("name"));
}

/// Verifies placeholder parsing and resolution for `iter with emoji in sql`.
#[test]
fn iter_with_emoji_in_sql() {
    let sql = "SELECT * FROM users WHERE status = '🎉' AND id = :id";
    let parts: Vec<_> = PlaceholderIter::new(sql).collect();
    assert!(parts.len() >= 2); // Should handle emoji correctly
}

/// Verifies placeholder parsing and resolution for `iter with chinese characters`.
#[test]
fn iter_with_chinese_characters() {
    let sql = "SELECT 你好 FROM users WHERE id = :id";
    let parts: Vec<_> = PlaceholderIter::new(sql).collect();
    assert!(parts.len() >= 2); // Should handle Chinese characters
}

/// Verifies placeholder parsing and resolution for `iter with multibyte in comment`.
#[test]
fn iter_with_multibyte_in_comment() {
    let sql = "-- Comment with café\nSELECT :id";
    let parts: Vec<_> = PlaceholderIter::new(sql).collect();
    assert_eq!(parts.len(), 2); // Comment, then placeholder
}

/// Verifies placeholder parsing and resolution for `iter with multibyte in string`.
#[test]
fn iter_with_multibyte_in_string() {
    let sql = "SELECT 'café' FROM users WHERE id = :id";
    let parts: Vec<_> = PlaceholderIter::new(sql).collect();
    assert!(parts.len() >= 2); // Should handle accented chars in strings
}

/// Verifies placeholder parsing and resolution for `iter with cyrillic`.
#[test]
fn iter_with_cyrillic() {
    let sql = "SELECT * FROM пользователи WHERE id = :id";
    let parts: Vec<_> = PlaceholderIter::new(sql).collect();
    assert!(parts.len() >= 2); // Should handle Cyrillic
}

/// Verifies placeholder parsing and resolution for `has placeholder with multibyte`.
#[test]
fn has_placeholder_with_multibyte() {
    assert!(has_named_placeholder("SELECT café WHERE id = :id"));
}

/// Verifies placeholder parsing and resolution for `resolve with multibyte`.
#[test]
fn resolve_with_multibyte() {
    use std::collections::HashMap;
    let sql = "SELECT * FROM café WHERE id = :id";
    let mut args = crate::commons::Arguments::default();
    let mut values = HashMap::new();
    values.insert("id".to_string(), crate::argvalue::ArgValue::new(42i32));

    let result = resolve_placeholders(sql, &mut args, &values, Dialect::Postgres);
    // Currently works but not guaranteed
    assert!(result.is_ok());
}
