fn main() {
    let backends = ["POSTGRES", "SQLITE", "MYSQL", "MARIADB"];
    let selected = backends
        .iter()
        .filter(|backend| std::env::var_os(format!("CARGO_FEATURE_{backend}")).is_some())
        .copied()
        .collect::<Vec<_>>();

    match selected.as_slice() {
        [_] => {}
        [] => fail(
            "Mool requires exactly one database backend feature: postgres, sqlite, mysql, or mariadb",
        ),
        _ => fail(&format!(
            "Mool database backend features are mutually exclusive; selected: {}",
            selected.join(", ").to_lowercase()
        )),
    }
}

fn fail(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(1)
}
