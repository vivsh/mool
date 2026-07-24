fn main() {
    println!("cargo::rustc-check-cfg=cfg(mool_has_backend)");
    let backends = ["POSTGRES", "SQLITE", "MYSQL", "MARIADB"];
    let selected = backends
        .iter()
        .filter(|backend| std::env::var_os(format!("CARGO_FEATURE_{backend}")).is_some())
        .copied()
        .collect::<Vec<_>>();

    if selected.len() > 1 {
        fail(&format!(
            "Mool database backend features are mutually exclusive; selected: {}",
            selected.join(", ").to_lowercase()
        ));
    }
    if selected.len() == 1 {
        println!("cargo::rustc-cfg=mool_has_backend");
    }
}

fn fail(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(1)
}
