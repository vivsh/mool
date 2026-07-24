fn main() {
    let registry = mool::migrations::MigrationRegistry::new();
    let _setup = mool::testing::setup(mool::DbConf::default()).with_migrations(&registry);
}
