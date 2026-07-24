use mool as db;

fn main() {
    let _setup = db::testing::setup(db::DbConf::default()).preserve();
}
