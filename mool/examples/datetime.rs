use mool as db;
use mool::Model;

#[derive(Debug, Clone, db::Model)]
#[table(name = "events")]
struct Event {
    id: i64,
    happened_at: chrono::DateTime<chrono::Utc>,
}

fn main() -> Result<(), db::QueryError> {
    let events = Event::table();
    let plan = db::from(&events)
        .filter(
            events
                .happened_at
                .lte(db::funcs::datetime::now::<chrono::DateTime<chrono::Utc>>()),
        )
        .filter(db::funcs::datetime::extract_year(events.happened_at.clone()).eq(db::val(2026)))
        .order_by(db::funcs::datetime::trunc_day(events.happened_at.clone()).desc())
        .all::<Event>()
        .plan()?;

    println!("{}", plan.sql);

    let shifted = db::from(&events)
        .scalar(db::funcs::datetime::add(
            events.happened_at.clone(),
            tokio::time::Duration::from_secs(300),
        ))
        .plan()?;
    println!("{}", shifted.sql);

    #[cfg(feature = "postgres")]
    {
        let local = db::from(&events)
            .scalar(db::funcs::postgres::datetime::at_time_zone(
                events.happened_at.clone(),
                "Asia/Kolkata",
            ))
            .plan()?;
        println!("{}", local.sql);
    }

    #[cfg(feature = "mysql")]
    {
        let calendar = db::from(&events)
            .scalar(db::funcs::mysql::datetime::add_calendar_months(
                events.happened_at.clone(),
                db::val(1_i64),
            ))
            .plan()?;
        println!("{}", calendar.sql);
    }

    #[cfg(feature = "mariadb")]
    {
        let calendar = db::from(&events)
            .scalar(db::funcs::mariadb::datetime::add_calendar_months(
                events.happened_at.clone(),
                db::val(1_i64),
            ))
            .plan()?;
        println!("{}", calendar.sql);
    }

    #[cfg(feature = "sqlite")]
    {
        let calendar = db::from(&events)
            .scalar(db::funcs::sqlite::datetime::add_calendar_months_floor(
                events.happened_at.clone(),
                db::val(1_i64),
            ))
            .plan()?;
        println!("{}", calendar.sql);
    }

    #[cfg(feature = "time")]
    {
        let current = db::funcs::datetime::now::<time::OffsetDateTime>();
        let _: db::Expr<time::OffsetDateTime> =
            db::funcs::datetime::add(current, time::Duration::minutes(15));
    }

    Ok(())
}
