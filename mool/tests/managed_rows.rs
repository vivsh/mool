//! Conformance tests for Record-backed Gaman managed rows.

use std::collections::BTreeMap;

use mool as db;

#[derive(Debug, Clone, db::Model)]
#[cfg_attr(feature = "postgres", table(name = "app_settings", schema = "config"))]
#[cfg_attr(not(feature = "postgres"), table(name = "app_settings"))]
struct AppSetting {
    #[cfg_attr(
        any(feature = "mysql", feature = "mariadb"),
        column(primary_key, name = "setting_key", type = "varchar(191)")
    )]
    #[cfg_attr(
        not(any(feature = "mysql", feature = "mariadb")),
        column(primary_key, name = "setting_key")
    )]
    key: String,
    value: serde_json::Value,
    #[column(default = "'system'", read_only)]
    source: String,
}

#[derive(Debug, Clone, db::Record, db::ManagedRecord)]
struct SettingKey {
    #[column(name = "setting_key")]
    key: String,
}

#[derive(Debug, Clone, db::Record, db::ManagedRecord)]
struct AppSettingSeed {
    #[column(flatten)]
    key: SettingKey,
    value: serde_json::Value,
    #[column(skip)]
    transient: bool,
}

#[derive(Debug, Clone, db::Record, db::ManagedRecord)]
struct DuplicateKey {
    #[column(name = "setting_key")]
    key: String,
}

#[derive(Debug, Clone, db::Record, db::ManagedRecord)]
struct DuplicateValue {
    #[column(name = "setting_key")]
    value: String,
}

#[derive(Debug, Clone, db::Record, db::ManagedRecord)]
struct DuplicateSeed {
    #[column(flatten)]
    key: DuplicateKey,
    #[column(flatten)]
    value: DuplicateValue,
}

#[derive(Debug, Clone, db::Record, db::ManagedRecord)]
struct InvalidValueSeed {
    #[column(name = "setting_key")]
    key: String,
    #[column(json)]
    value: BTreeMap<(u8, u8), String>,
}

#[derive(Debug, Clone, db::Record, db::ManagedRecord)]
struct MissingKeySeed {
    value: serde_json::Value,
}

fn setting(key: &str, value: serde_json::Value) -> AppSettingSeed {
    AppSettingSeed {
        key: SettingKey {
            key: key.to_string(),
        },
        value,
        transient: true,
    }
}

fn desired_schema(rows: impl IntoIterator<Item = AppSettingSeed>) -> db::schema::Schema {
    db::schema()
        .managed_rows::<AppSetting>(rows)
        .model::<AppSetting>()
        .build()
        .expect("valid desired schema")
}

/// Verifies derived records serialize insertable fields with physical column names.
#[test]
fn record_managed_values_use_physical_insert_columns() {
    let seed = setting("mail.reply_to", serde_json::json!("support@example.com"));
    assert!(seed.transient);
    let values = db::ManagedRecord::managed_values(&seed).expect("serializable record");

    assert_eq!(
        values.get("setting_key"),
        Some(&serde_json::json!("mail.reply_to"))
    );
    assert_eq!(
        values.get("value"),
        Some(&serde_json::json!("support@example.com"))
    );
    assert!(!values.contains_key("transient"));
}

/// Verifies read-only model fields remain represented in normal model values.
#[test]
fn model_read_only_fields_remain_available_to_application_code() {
    let setting = AppSetting {
        key: "mail.reply_to".to_string(),
        value: serde_json::json!("support@example.com"),
        source: "system".to_string(),
    };

    assert_eq!(setting.key, "mail.reply_to");
    assert_eq!(setting.value, serde_json::json!("support@example.com"));
    assert_eq!(setting.source, "system");
}

/// Verifies model registration can follow managed rows and preserve target identity.
#[test]
fn schema_builder_registers_managed_rows_for_the_model_target() {
    let schema = desired_schema([setting(
        "mail.reply_to",
        serde_json::json!("support@example.com"),
    )]);
    let rows = schema
        .managed_rows
        .get(app_settings_table())
        .expect("managed rows for model target");

    assert_eq!(rows.rows.len(), 1);
    assert_eq!(
        rows.rows[0].values["setting_key"].0,
        serde_json::json!("mail.reply_to")
    );
}

/// Verifies missing target tables are rejected only at the schema-builder terminal.
#[test]
fn schema_builder_defers_missing_managed_row_target() {
    let error = db::schema()
        .managed_rows::<AppSetting>([setting(
            "mail.reply_to",
            serde_json::json!("support@example.com"),
        )])
        .build()
        .expect_err("missing model table must fail");

    assert!(error.to_string().contains(app_settings_table()));
}

/// Verifies flattened duplicate columns and invalid JSON values fail at the builder terminal.
#[test]
fn schema_builder_defers_managed_record_conversion_errors() {
    let duplicate = db::schema()
        .model::<AppSetting>()
        .managed_rows::<AppSetting>([DuplicateSeed {
            key: DuplicateKey {
                key: "mail.reply_to".to_string(),
            },
            value: DuplicateValue {
                value: "duplicate".to_string(),
            },
        }])
        .build()
        .expect_err("duplicate flattened columns must fail");
    assert!(
        duplicate
            .to_string()
            .contains("duplicate column 'setting_key'")
    );

    let invalid_value = db::schema()
        .model::<AppSetting>()
        .managed_rows::<AppSetting>([InvalidValueSeed {
            key: "mail.reply_to".to_string(),
            value: BTreeMap::from([((1, 2), "support@example.com".to_string())]),
        }])
        .build()
        .expect_err("non-string map keys must fail");
    assert!(
        invalid_value
            .to_string()
            .contains("cannot serialize managed column 'value'")
    );
}

/// Verifies Gaman validates managed-row identities against the target model key.
#[test]
fn schema_builder_rejects_managed_rows_without_a_model_key() {
    let error = db::schema()
        .model::<AppSetting>()
        .managed_rows::<AppSetting>([MissingKeySeed {
            value: serde_json::json!("support@example.com"),
        }])
        .build()
        .expect_err("managed rows require the target key");

    assert!(error.to_string().contains("non-null primary or unique key"));
}

/// Verifies changes and removals in the complete managed set generate update and delete operations.
#[test]
fn managed_rows_generate_updates_and_deletes_for_complete_sets() {
    let initial = desired_schema([
        setting("mail.reply_to", serde_json::json!("support@example.com")),
        setting("mail.sender", serde_json::json!("Mool")),
    ]);
    let planner = db::gaman::core::OfflinePlanner::new(active_dialect());
    let baseline = planner
        .make_migration(initial, &[])
        .expect("initial migration plans")
        .expect("initial migration exists");
    let changed = desired_schema([setting(
        "mail.reply_to",
        serde_json::json!("help@example.com"),
    )]);
    let pending = db::gaman::core::OfflinePlanner::new(active_dialect())
        .from_migrations(vec![baseline.clone()])
        .make_migration(changed.clone(), &[])
        .expect_err("managed-row deletion requires review");
    let decisions = match pending {
        db::gaman::core::OfflineError::NeedsInput(clarifications) => clarifications
            .into_iter()
            .map(|clarification| db::gaman::core::Decision {
                clarification_id: clarification.id,
                answer: db::gaman::core::Answer::AcceptRisk,
            })
            .collect::<Vec<_>>(),
        error => panic!("expected a deletion clarification: {error}"),
    };
    let migration = db::gaman::core::OfflinePlanner::new(active_dialect())
        .from_migrations(vec![baseline])
        .make_migration(changed, &decisions)
        .expect("managed-row changes plan")
        .expect("managed-row migration exists");

    assert!(migration.operations.iter().any(|operation| matches!(
        operation,
        db::gaman::schema::Operation::UpdateRow { table_name, .. }
            if table_name == app_settings_table()
    )));
    assert!(migration.operations.iter().any(|operation| matches!(
        operation,
        db::gaman::schema::Operation::DeleteRow { table_name, .. }
            if table_name == app_settings_table()
    )));
}

fn active_dialect() -> db::gaman::core::Dialect {
    #[cfg(feature = "postgres")]
    return db::gaman::core::Dialect::Postgres;
    #[cfg(feature = "sqlite")]
    return db::gaman::core::Dialect::Sqlite;
    #[cfg(feature = "mysql")]
    return db::gaman::core::Dialect::Mysql;
    #[cfg(feature = "mariadb")]
    return db::gaman::core::Dialect::Mariadb;
}

fn app_settings_table() -> &'static str {
    #[cfg(feature = "postgres")]
    return "config.app_settings";
    #[cfg(not(feature = "postgres"))]
    "app_settings"
}
