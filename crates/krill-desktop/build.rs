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
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rerun-if-changed=../../resources/windows/krill.ico");
        let parts: Vec<u64> = version.split('.').map(|p| p.parse().unwrap()).collect();
        let packed = (parts[0] << 48) | (parts[1] << 32) | (parts[2] << 16);
        winresource::WindowsResource::new()
            .set_icon("../../resources/windows/krill.ico")
            .set("FileDescription", "Krill Desktop")
            .set("ProductName", "Krill Desktop")
            .set("OriginalFilename", "Krill.Desktop.exe")
            .set("FileVersion", &version)
            .set("ProductVersion", &version)
            .set_version_info(winresource::VersionInfo::FILEVERSION, packed)
            .set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed)
            .compile()
            .expect("Windows icon and version must be embedded");
        // GPUI already embeds the PerMonitorV2 / Windows 10 / Common Controls
        // manifest. Embedding a second ID 1 manifest causes CVTRES CVT1100.
    }
}
