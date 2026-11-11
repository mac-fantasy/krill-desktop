//! Build-only package helper; never linked into or shipped with the client.
//! Emit a metadata-only DLL for 0.1.x updaters' AssemblyName version check.
use anyhow::{Context, Result};
use dotnetdll::prelude::*;

fn marker(version: krill_core::release::Version) -> Result<Vec<u8>> {
    let mut resolution = Resolution::new(Module::new("Krill.Desktop.dll"));
    let mut assembly = Assembly::new("Krill.Desktop");
    assembly.version = dotnetdll::resolved::assembly::Version {
        major: version.0,
        minor: version.1,
        build: version.2,
        revision: 0,
    };
    resolution.assembly = Some(assembly);
    Ok(resolution.write(WriteOptions::default())?)
}
fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let version = args
        .next()
        .context("Usage: krill-package X.Y.Z output-directory")?
        .parse::<krill_core::release::Version>()
        .map_err(|_| anyhow::anyhow!("Invalid package version"))?;
    let directory = std::path::PathBuf::from(args.next().context("Output directory required")?);
    std::fs::create_dir_all(&directory)?;
    std::fs::write(directory.join("Krill.Desktop.dll"), marker(version)?)?;
    std::fs::write(
        directory.join("krill-package.json"),
        format!(
            "{{\"version\":\"{version}\",\"platform\":\"win-x64\",\"executable\":\"Krill.Desktop.exe\"}}\n"
        ),
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compatibility_marker_contains_only_version_identity_and_no_managed_application() {
        let bytes = marker(krill_core::release::Version(2, 10, 3)).unwrap();
        let resolution = Resolution::parse(&bytes, ReadOptions::default()).unwrap();
        let assembly = resolution.assembly.unwrap();
        assert_eq!(assembly.name, "Krill.Desktop");
        assert_eq!(
            (
                assembly.version.major,
                assembly.version.minor,
                assembly.version.build
            ),
            (2, 10, 3)
        );
        assert!(resolution.assembly_references.is_empty());
        assert!(bytes.len() < 8192);
    }
}
