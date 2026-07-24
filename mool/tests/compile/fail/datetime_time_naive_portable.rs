use mool as db;

fn main() {
    let naive = db::val(time::PrimitiveDateTime::MIN);
    let _ = db::funcs::datetime::trunc_day(naive);
}
