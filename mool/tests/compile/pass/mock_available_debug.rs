#![allow(dead_code)]

fn main() {
    let mut session = mool::mock::MockDbSession::new();
    session.plan_execute_ok("INSERT INTO posts DEFAULT VALUES", 1);
}
