use mool as db;

fn main() {
    let local = db::var::<chrono::DateTime<chrono::Local>>();
    let _ = db::funcs::datetime::trunc_day(local);

    let fixed = db::var::<chrono::DateTime<chrono::FixedOffset>>();
    let _ = db::funcs::datetime::extract_year(fixed);
}
