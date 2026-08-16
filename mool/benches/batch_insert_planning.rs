use std::hint::black_box;
use std::time::{Duration, Instant};

use mool as db;
use mool::Model;
use mool::backend::PostgresUnnestExt;

#[derive(Clone, db::Model)]
#[table(name = "benchmark_rows")]
struct BenchmarkRow {
    id: i64,
    title: String,
    active: bool,
}

#[derive(Clone, db::Model)]
#[table(name = "benchmark_json_rows")]
struct BenchmarkJsonRow {
    id: i64,
    #[column(type = "jsonb")]
    payload: serde_json::Value,
}

fn rows(count: usize) -> Vec<BenchmarkRow> {
    (0..count)
        .map(|index| BenchmarkRow {
            id: index as i64,
            title: format!("row-{index}"),
            active: index % 2 == 0,
        })
        .collect()
}

fn json_rows(count: usize) -> Vec<BenchmarkJsonRow> {
    (0..count)
        .map(|index| BenchmarkJsonRow {
            id: index as i64,
            payload: serde_json::json!({ "index": index, "active": index % 2 == 0 }),
        })
        .collect()
}

fn measure<F>(iterations: usize, mut operation: F) -> Result<Duration, db::QueryError>
where
    F: FnMut() -> Result<db::QueryPlan, db::QueryError>,
{
    let started = Instant::now();
    for _ in 0..iterations {
        black_box(operation()?);
    }
    Ok(started.elapsed())
}

fn benchmark(count: usize) -> Result<(), db::QueryError> {
    let table = BenchmarkRow::table();
    let rows = rows(count);
    let json_table = BenchmarkJsonRow::table();
    let json_rows = json_rows(count);
    let iterations = if count < 1_000 { 100 } else { 20 };
    let query = measure(iterations, || {
        db::from(&table)
            .filter(table.active.eq(db::val(true)))
            .filter(table.id.gt(db::val(100_i64)))
            .all::<BenchmarkRow>()
            .plan()
    })?;
    let values = measure(iterations, || db::from(&table).insert_many(&rows).plan())?;
    let json_values = measure(iterations, || {
        db::from(&json_table).insert_many(&json_rows).plan()
    })?;
    let unnest = measure(iterations, || {
        db::from(&table).insert_many(&rows).using_unnest().plan()
    })?;
    println!(
        "rows={count} iterations={iterations} query={query:?} values={values:?} json_values={json_values:?} unnest={unnest:?}"
    );
    Ok(())
}

fn main() -> Result<(), db::QueryError> {
    for count in [100, 1_000, 10_000] {
        benchmark(count)?;
    }
    Ok(())
}
