fn main() {
    println!("cargo:rerun-if-env-changed=KRILL_VERSION");
    let version = std::env::var("KRILL_VERSION")
        .unwrap_or_else(|_| std::env::var("CARGO_PKG_VERSION").unwrap());
    assert!(
        version.split('.').count() == 3
            && version.split('.').all(|p| !p.is_empty()
                && p.bytes().all(|b| b.is_ascii_digit())
                && p.parse::<u16>().is_ok()
                && (p.len() == 1 || !p.starts_with('0'))),
        "KRILL_VERSION must be X.Y.Z"
    );
    println!("cargo:rustc-env=KRILL_BUILD_VERSION={version}");
}
