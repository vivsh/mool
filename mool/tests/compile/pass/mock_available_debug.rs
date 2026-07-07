#![allow(dead_code)]

fn main() {
    let mut session = mool::mock::MockDBSession::new();
    session.plan_execute_ok("INSERT", 1);
}
