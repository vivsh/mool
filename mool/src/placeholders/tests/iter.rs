use super::*;

/// Verifies placeholder parsing and resolution for `empty string`.
#[test]
fn empty_string() {
    assert_eq!(collect_parts(""), vec![]);
}

/// Verifies placeholder parsing and resolution for `no placeholders`.
#[test]
fn no_placeholders() {
    let parts = collect_parts("SELECT * FROM users");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT * FROM users")]);
}

/// Verifies placeholder parsing and resolution for `single placeholder`.
#[test]
fn single_placeholder() {
    let parts = collect_parts("SELECT * FROM users WHERE id = :id");
    assert_eq!(
        parts,
        vec![
            PlaceholderPart::Sql("SELECT * FROM users WHERE id = "),
            PlaceholderPart::Placeholder("id"),
        ]
    );
}

/// Verifies placeholder parsing and resolution for `multiple placeholders`.
#[test]
fn multiple_placeholders() {
    let parts = collect_parts("SELECT * FROM users WHERE id = :id AND name = :name");
    assert_eq!(
        parts,
        vec![
            PlaceholderPart::Sql("SELECT * FROM users WHERE id = "),
            PlaceholderPart::Placeholder("id"),
            PlaceholderPart::Sql(" AND name = "),
            PlaceholderPart::Placeholder("name"),
        ]
    );
}

/// Verifies placeholder parsing and resolution for `placeholder at start`.
#[test]
fn placeholder_at_start() {
    let parts = collect_parts(":id");
    assert_eq!(parts, vec![PlaceholderPart::Placeholder("id"),]);
}

/// Verifies placeholder parsing and resolution for `placeholder at end`.
#[test]
fn placeholder_at_end() {
    let parts = collect_parts("SELECT :id");
    assert_eq!(
        parts,
        vec![
            PlaceholderPart::Sql("SELECT "),
            PlaceholderPart::Placeholder("id"),
        ]
    );
}

/// Verifies placeholder parsing and resolution for `placeholder names with numbers and underscores`.
#[test]
fn placeholder_names_with_numbers_and_underscores() {
    let parts = collect_parts(":id_1 :user2 :_private");
    assert_eq!(
        parts,
        vec![
            PlaceholderPart::Placeholder("id_1"),
            PlaceholderPart::Sql(" "),
            PlaceholderPart::Placeholder("user2"),
            PlaceholderPart::Sql(" "),
            PlaceholderPart::Placeholder("_private"),
        ]
    );
}

/// Verifies placeholder parsing and resolution for `double colon not placeholder`.
#[test]
fn double_colon_not_placeholder() {
    // PostgreSQL cast operator ::
    let parts = collect_parts("SELECT id::integer");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT id::integer")]);
}

/// Verifies placeholder parsing and resolution for `colon at end of string`.
#[test]
fn colon_at_end_of_string() {
    let parts = collect_parts("SELECT :");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT :")]);
}

/// Verifies placeholder parsing and resolution for `colon followed by non name`.
#[test]
fn colon_followed_by_non_name() {
    let parts = collect_parts("SELECT :123");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT :123")]);
}

/// Verifies placeholder parsing and resolution for `word char before colon not placeholder`.
#[test]
fn word_char_before_colon_not_placeholder() {
    let parts = collect_parts("abc:param");
    assert_eq!(parts, vec![PlaceholderPart::Sql("abc:param")]);
}

/// Verifies placeholder parsing and resolution for `placeholder in single quotes ignored`.
#[test]
fn placeholder_in_single_quotes_ignored() {
    let parts = collect_parts("SELECT 'text :param' FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT 'text :param' FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `placeholder in double quotes ignored`.
#[test]
fn placeholder_in_double_quotes_ignored() {
    let parts = collect_parts("SELECT \"col:param\" FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT \"col:param\" FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `placeholder in backticks ignored`.
#[test]
fn placeholder_in_backticks_ignored() {
    let parts = collect_parts("SELECT `col:param` FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT `col:param` FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `placeholder in bracket ident ignored`.
#[test]
fn placeholder_in_bracket_ident_ignored() {
    let parts = collect_parts("SELECT [col:param] FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT [col:param] FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `escaped single quote with doubling`.
#[test]
fn escaped_single_quote_with_doubling() {
    let parts = collect_parts("SELECT 'don''t :param' FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT 'don''t :param' FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `escaped double quote with doubling`.
#[test]
fn escaped_double_quote_with_doubling() {
    let parts = collect_parts("SELECT \"col\"\"name:param\" FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT \"col\"\"name:param\" FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `escaped backtick with doubling`.
#[test]
fn escaped_backtick_with_doubling() {
    let parts = collect_parts("SELECT `col``name:param` FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT `col``name:param` FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `backslash escape single quote`.
#[test]
fn backslash_escape_single_quote() {
    let parts = collect_parts("SELECT 'don\\'t :param' FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT 'don\\'t :param' FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `backslash escape double quote`.
#[test]
fn backslash_escape_double_quote() {
    let parts = collect_parts("SELECT \"col\\\"name:param\" FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT \"col\\\"name:param\" FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `backslash escape backtick`.
#[test]
fn backslash_escape_backtick() {
    let parts = collect_parts("SELECT `col\\`name:param` FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT `col\\`name:param` FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `backslash at end of single quote`.
#[test]
fn backslash_at_end_of_single_quote() {
    // 'text\' - the backslash escapes the closing quote, so string is unclosed
    let parts = collect_parts("SELECT 'text\\' :param");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT 'text\\' :param")]);
}

/// Verifies placeholder parsing and resolution for `backslash backslash in quote`.
#[test]
fn backslash_backslash_in_quote() {
    let parts = collect_parts("SELECT 'path\\\\:param' FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT 'path\\\\:param' FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `line comment double dash`.
#[test]
fn line_comment_double_dash() {
    let parts = collect_parts("SELECT * -- :param\nFROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT * -- :param\nFROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `line comment hash`.
#[test]
fn line_comment_hash() {
    let parts = collect_parts("SELECT * # :param\nFROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT * # :param\nFROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `line comment at end no newline`.
#[test]
fn line_comment_at_end_no_newline() {
    let parts = collect_parts("SELECT * -- :param");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT * -- :param")]);
}

/// Verifies placeholder parsing and resolution for `block comment`.
#[test]
fn block_comment() {
    let parts = collect_parts("SELECT * /* :param */ FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT * /* :param */ FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `block comment multiline`.
#[test]
fn block_comment_multiline() {
    let parts = collect_parts("SELECT * /* line1\n:param\nline2 */ FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql(
            "SELECT * /* line1\n:param\nline2 */ FROM t"
        )]
    );
}

/// Verifies placeholder parsing and resolution for `block comment not closed`.
#[test]
fn block_comment_not_closed() {
    let parts = collect_parts("SELECT * /* :param");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT * /* :param")]);
}

/// Verifies placeholder parsing and resolution for `dollar quote empty tag`.
#[test]
fn dollar_quote_empty_tag() {
    let parts = collect_parts("SELECT $$:param$$ FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT $$:param$$ FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `dollar quote with tag`.
#[test]
fn dollar_quote_with_tag() {
    let parts = collect_parts("SELECT $tag$:param$tag$ FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT $tag$:param$tag$ FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `dollar quote different tags not matched`.
#[test]
fn dollar_quote_different_tags_not_matched() {
    let parts = collect_parts("SELECT $a$text$b$ FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT $a$text$b$ FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `dollar quote tag must be identifier`.
#[test]
fn dollar_quote_tag_must_be_identifier() {
    let parts = collect_parts("SELECT $123$ FROM t");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT $123$ FROM t")]);
}

/// Verifies placeholder parsing and resolution for `dollar quote nested dollar signs`.
#[test]
fn dollar_quote_nested_dollar_signs() {
    let parts = collect_parts("SELECT $$text $ more$$ FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT $$text $ more$$ FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `dollar quote tag prefix matching`.
#[test]
fn dollar_quote_tag_prefix_matching() {
    // $tag$ should not match $tagg$
    let parts = collect_parts("SELECT $tag$text$tagg$ FROM t");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("SELECT $tag$text$tagg$ FROM t")]
    );
}

/// Verifies placeholder parsing and resolution for `placeholder before and after quotes`.
#[test]
fn placeholder_before_and_after_quotes() {
    let parts = collect_parts(":a 'text' :b");
    assert_eq!(
        parts,
        vec![
            PlaceholderPart::Placeholder("a"),
            PlaceholderPart::Sql(" 'text' "),
            PlaceholderPart::Placeholder("b"),
        ]
    );
}

/// Verifies placeholder parsing and resolution for `complex query with multiple features`.
#[test]
fn complex_query_with_multiple_features() {
    let sql = r#"
            SELECT * FROM users
            WHERE id = :id -- user id
              AND name = 'O''Neil :fake'
              AND email = :email /* :notthis */
              AND data = $${"key": ":value"}$$
              AND status::text = :status
        "#;
    let parts = parts_to_strings(collect_parts(sql));
    assert!(parts.contains(&"PARAM:id".to_string()));
    assert!(parts.contains(&"PARAM:email".to_string()));
    assert!(parts.contains(&"PARAM:status".to_string()));
    assert!(!parts.iter().any(|s| s.contains("PARAM:fake")));
    assert!(!parts.iter().any(|s| s.contains("PARAM:notthis")));
    assert!(!parts.iter().any(|s| s.contains("PARAM:value")));
}

/// Verifies placeholder parsing and resolution for `placeholder after various punctuation`.
#[test]
fn placeholder_after_various_punctuation() {
    let parts = collect_parts("(:a, :b):c {:d}");
    assert_eq!(
        parts_to_strings(parts),
        vec![
            "SQL:(".to_string(),
            "PARAM:a".to_string(),
            "SQL:, ".to_string(),
            "PARAM:b".to_string(),
            "SQL:)".to_string(),
            "PARAM:c".to_string(),
            "SQL: {".to_string(),
            "PARAM:d".to_string(),
            "SQL:}".to_string(),
        ]
    );
}

/// Verifies placeholder parsing and resolution for `unclosed single quote`.
#[test]
fn unclosed_single_quote() {
    let parts = collect_parts("SELECT ':param");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT ':param")]);
}

/// Verifies placeholder parsing and resolution for `unclosed double quote`.
#[test]
fn unclosed_double_quote() {
    let parts = collect_parts("SELECT \":param");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT \":param")]);
}

/// Verifies placeholder parsing and resolution for `unclosed backtick`.
#[test]
fn unclosed_backtick() {
    let parts = collect_parts("SELECT `:param");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT `:param")]);
}

/// Verifies placeholder parsing and resolution for `unclosed bracket ident`.
#[test]
fn unclosed_bracket_ident() {
    let parts = collect_parts("SELECT [:param");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT [:param")]);
}

/// Verifies placeholder parsing and resolution for `unclosed dollar quote`.
#[test]
fn unclosed_dollar_quote() {
    let parts = collect_parts("SELECT $$:param");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT $$:param")]);
}

/// Verifies placeholder parsing and resolution for `empty placeholder name`.
#[test]
fn empty_placeholder_name() {
    // : followed by space
    let parts = collect_parts("SELECT : FROM t");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT : FROM t")]);
}

/// Verifies placeholder parsing and resolution for `consecutive placeholders`.
#[test]
fn consecutive_placeholders() {
    let parts = collect_parts(":a:b:c");
    assert_eq!(
        parts_to_strings(parts),
        vec![
            "PARAM:a".to_string(),
            "PARAM:b".to_string(),
            "PARAM:c".to_string(),
        ]
    );
}

/// Verifies placeholder parsing and resolution for `placeholder with operators`.
#[test]
fn placeholder_with_operators() {
    let parts = collect_parts("SELECT :a+:b*:c");
    assert_eq!(
        parts_to_strings(parts),
        vec![
            "SQL:SELECT ".to_string(),
            "PARAM:a".to_string(),
            "SQL:+".to_string(),
            "PARAM:b".to_string(),
            "SQL:*".to_string(),
            "PARAM:c".to_string(),
        ]
    );
}

/// Verifies placeholder parsing and resolution for `all quote types in sequence`.
#[test]
fn all_quote_types_in_sequence() {
    let parts = collect_parts("':a' \":b\" `:c` [:d] $$:e$$");
    assert_eq!(
        parts,
        vec![PlaceholderPart::Sql("':a' \":b\" `:c` [:d] $$:e$$")]
    );
}

/// Verifies placeholder parsing and resolution for `mixed comment types`.
#[test]
fn mixed_comment_types() {
    let parts = collect_parts("-- :a\n/* :b */ # :c\n:d");
    assert_eq!(
        parts,
        vec![
            PlaceholderPart::Sql("-- :a\n/* :b */ # :c\n"),
            PlaceholderPart::Placeholder("d"),
        ]
    );
}

/// Verifies placeholder parsing and resolution for `backslash at string end boundary`.
#[test]
fn backslash_at_string_end_boundary() {
    // Backslash at end of input inside quote
    let parts = collect_parts("SELECT 'text\\");
    assert_eq!(parts, vec![PlaceholderPart::Sql("SELECT 'text\\")]);
}

/// Verifies placeholder parsing and resolution for `single colon`.
#[test]
fn single_colon() {
    let parts = collect_parts(":");
    assert_eq!(parts, vec![PlaceholderPart::Sql(":")]);
}

/// Verifies placeholder parsing and resolution for `only placeholder`.
#[test]
fn only_placeholder() {
    let parts = collect_parts(":param");
    assert_eq!(parts, vec![PlaceholderPart::Placeholder("param")]);
}

/// Verifies placeholder parsing and resolution for `placeholder uppercase letters`.
#[test]
fn placeholder_uppercase_letters() {
    let parts = collect_parts(":USER_ID :UserName");
    assert_eq!(
        parts,
        vec![
            PlaceholderPart::Placeholder("USER_ID"),
            PlaceholderPart::Sql(" "),
            PlaceholderPart::Placeholder("UserName"),
        ]
    );
}

/// Verifies placeholder parsing and resolution for `dollar sign not quote start`.
#[test]
fn dollar_sign_not_quote_start() {
    let parts = collect_parts("SELECT $ :param FROM t");
    assert_eq!(
        parts,
        vec![
            PlaceholderPart::Sql("SELECT $ "),
            PlaceholderPart::Placeholder("param"),
            PlaceholderPart::Sql(" FROM t"),
        ]
    );
}

/// Verifies placeholder parsing and resolution for `star slash outside comment`.
#[test]
fn star_slash_outside_comment() {
    let parts = collect_parts("SELECT */ :param");
    assert_eq!(
        parts,
        vec![
            PlaceholderPart::Sql("SELECT */ "),
            PlaceholderPart::Placeholder("param"),
        ]
    );
}

/// Verifies placeholder parsing and resolution for `dash not double`.
#[test]
fn dash_not_double() {
    let parts = collect_parts("SELECT - :param");
    assert_eq!(
        parts,
        vec![
            PlaceholderPart::Sql("SELECT - "),
            PlaceholderPart::Placeholder("param"),
        ]
    );
}

/// Verifies placeholder parsing and resolution for `has placeholder returns true`.
#[test]
fn has_placeholder_returns_true() {
    assert!(has_named_placeholder("SELECT * WHERE id = :id"));
    assert!(has_named_placeholder(":param"));
    assert!(has_named_placeholder("text :param more"));
}

/// Verifies placeholder parsing and resolution for `has placeholder returns false`.
#[test]
fn has_placeholder_returns_false() {
    assert!(!has_named_placeholder("SELECT * FROM users"));
    assert!(!has_named_placeholder(""));
    assert!(!has_named_placeholder("SELECT ':param' FROM t"));
    assert!(!has_named_placeholder("-- :param"));
    assert!(!has_named_placeholder("id::integer"));
}

/// Verifies placeholder parsing and resolution for `has placeholder short circuits`.
#[test]
fn has_placeholder_short_circuits() {
    // Should stop at first placeholder without scanning entire string
    assert!(has_named_placeholder(":first :second :third"));
}

/// Verifies placeholder parsing and resolution for `has placeholder incomplete single quote`.
#[test]
fn has_placeholder_incomplete_single_quote() {
    // Unclosed quote - placeholder before quote should be found
    assert!(has_named_placeholder(":param 'unclosed"));
    // Unclosed quote - placeholder inside quote should be ignored
    assert!(!has_named_placeholder("'unclosed :param"));
}

/// Verifies placeholder parsing and resolution for `has placeholder incomplete double quote`.
#[test]
fn has_placeholder_incomplete_double_quote() {
    // Unclosed double quote
    assert!(has_named_placeholder(":param \"unclosed"));
    assert!(!has_named_placeholder("\"unclosed :param"));
}

/// Verifies placeholder parsing and resolution for `has placeholder incomplete backtick`.
#[test]
fn has_placeholder_incomplete_backtick() {
    // Unclosed backtick
    assert!(has_named_placeholder(":param `unclosed"));
    assert!(!has_named_placeholder("`unclosed :param"));
}

/// Verifies placeholder parsing and resolution for `has placeholder incomplete bracket`.
#[test]
fn has_placeholder_incomplete_bracket() {
    // Unclosed bracket identifier
    assert!(has_named_placeholder(":param [unclosed"));
    assert!(!has_named_placeholder("[unclosed :param"));
}

/// Verifies placeholder parsing and resolution for `has placeholder incomplete line comment`.
#[test]
fn has_placeholder_incomplete_line_comment() {
    // Line comment without newline - placeholder in comment ignored
    assert!(!has_named_placeholder("-- :param"));
    assert!(!has_named_placeholder("# :param"));
    // Placeholder before comment found
    assert!(has_named_placeholder(":param --"));
}

/// Verifies placeholder parsing and resolution for `has placeholder incomplete block comment`.
#[test]
fn has_placeholder_incomplete_block_comment() {
    // Unclosed block comment - placeholder inside ignored
    assert!(!has_named_placeholder("/* :param"));
    // Placeholder before unclosed comment found
    assert!(has_named_placeholder(":param /*"));
}

/// Verifies placeholder parsing and resolution for `has placeholder incomplete dollar quote`.
#[test]
fn has_placeholder_incomplete_dollar_quote() {
    // Incomplete dollar quote
    assert!(has_named_placeholder(":param $$text"));
    assert!(has_named_placeholder(":param $tag$text"));
    // Placeholder inside unclosed dollar quote
    assert!(!has_named_placeholder("$$:param"));
    assert!(!has_named_placeholder("$tag$:param"));
}

/// Verifies placeholder parsing and resolution for `has placeholder partial placeholder`.
#[test]
fn has_placeholder_partial_placeholder() {
    // Colon at end of string
    assert!(!has_named_placeholder("SELECT :"));
    // Colon followed by non-name char
    assert!(!has_named_placeholder("SELECT :123"));
    // Incomplete placeholder name is still a placeholder
    assert!(has_named_placeholder("SELECT :p"));
}

/// Verifies placeholder parsing and resolution for `has placeholder escaped quote incomplete`.
#[test]
fn has_placeholder_escaped_quote_incomplete() {
    // Escaped quote at end - string still open
    assert!(!has_named_placeholder("'text\\' :param"));
    // Note: 'text\' leaves quote open, :param is inside the string context
}

/// Verifies placeholder parsing and resolution for `has placeholder mixed incomplete`.
#[test]
fn has_placeholder_mixed_incomplete() {
    // Multiple incomplete constructs
    assert!(has_named_placeholder(":a /* :b"));
    assert!(has_named_placeholder(":a 'b"));
    assert!(!has_named_placeholder("'a :b /* :c"));
    assert!(!has_named_placeholder("/* 'quoted :param"));
}

/// Verifies placeholder parsing and resolution for `has placeholder empty and whitespace`.
#[test]
fn has_placeholder_empty_and_whitespace() {
    assert!(!has_named_placeholder(""));
    assert!(!has_named_placeholder("   "));
    assert!(!has_named_placeholder("\n\t"));
}

/// Verifies placeholder parsing and resolution for `has placeholder only special chars`.
#[test]
fn has_placeholder_only_special_chars() {
    assert!(!has_named_placeholder("::::"));
    assert!(!has_named_placeholder("/* */ -- "));
    assert!(!has_named_placeholder("'''' \"\" ``"));
}
