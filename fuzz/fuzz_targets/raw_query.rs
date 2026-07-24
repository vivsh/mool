#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|sql: String| {
    let _ = mool::query(&sql).to_statement();
});
