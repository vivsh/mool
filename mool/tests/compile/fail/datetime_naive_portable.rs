use mool as db;

fn main() {
    let naive = db::val(chrono::NaiveDateTime::default());
    let _ = db::funcs::datetime::trunc_day(naive);
}
