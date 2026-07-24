use mool as db;
use mool::Model;
#[cfg(all(feature = "postgres", feature = "time"))]
use mool::backend::PostgresUnnestExt;

#[derive(Debug, Clone, db::Model)]
#[table(name = "events")]
struct Event {
    id: i64,
    happened_at: chrono::DateTime<chrono::Utc>,
    optional_at: Option<chrono::DateTime<chrono::Utc>>,
    happened_on: chrono::NaiveDate,
    local_at: chrono::NaiveDateTime,
}

#[cfg(feature = "time")]
#[derive(Debug, Clone, db::Model)]
#[table(name = "time_events")]
struct TimeEvent {
    id: i64,
    happened_at: time::OffsetDateTime,
    optional_at: Option<time::OffsetDateTime>,
    happened_on: time::Date,
    local_at: time::PrimitiveDateTime,
}

fn assert_expr<T>(_expression: db::Expr<T>) {}

fn main() {
    let events = Event::table();
    assert_expr::<chrono::DateTime<chrono::Utc>>(db::funcs::datetime::trunc_day(
        events.happened_at.clone(),
    ));
    assert_expr::<Option<i32>>(db::funcs::datetime::extract_year(
        events.optional_at.clone(),
    ));
    assert_expr::<chrono::NaiveDate>(db::funcs::datetime::date(
        events.happened_at.clone(),
    ));
    assert_expr::<Option<i64>>(db::funcs::datetime::diff_seconds(
        events.happened_at.clone(),
        events.optional_at.clone(),
    ));
    assert_expr::<chrono::DateTime<chrono::Utc>>(db::funcs::datetime::add(
        events.happened_at.clone(),
        tokio::time::Duration::from_secs(1),
    ));

    #[cfg(feature = "time")]
    {
        let events = TimeEvent::table();
        assert_expr::<time::OffsetDateTime>(db::funcs::datetime::trunc_hour(
            events.happened_at.clone(),
        ));
        assert_expr::<Option<time::OffsetDateTime>>(db::funcs::datetime::add(
            events.optional_at.clone(),
            time::Duration::seconds(1),
        ));
        assert_expr::<time::Date>(db::funcs::datetime::date(events.happened_at.clone()));

        #[cfg(feature = "postgres")]
        {
            let rows = [TimeEvent {
                id: 1,
                happened_at: time::OffsetDateTime::UNIX_EPOCH,
                optional_at: None,
                happened_on: time::Date::MIN,
                local_at: time::PrimitiveDateTime::MIN,
            }];
            let _ = db::from(&events)
                .batch_insert(&rows)
                .using_unnest()
                .plan();
        }
    }
}
