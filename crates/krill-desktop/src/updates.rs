use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use krill_core::release::{Asset, LATEST_URL, Release, Version};
use reqwest::{blocking::Client, redirect::Policy};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

pub const CURRENT_VERSION: &str = env!("KRILL_BUILD_VERSION");
pub const PLATFORM: &str = if cfg!(target_os = "macos") {
    "macos-arm64"
} else {
    "win-x64"
};
#[derive(Clone, Debug)]
pub struct AvailableUpdate {
    pub version: Version,
    pub asset: Asset,
}
pub struct UpdateState {
    pub status: String,
    pub busy: bool,
    pub available: Option<AvailableUpdate>,
    pub last_check: Option<DateTime<Utc>>,
}
impl Default for UpdateState {
    fn default() -> Self {
        Self {
            status: "启动后自动检查更新，每 6 小时检查一次。".into(),
            busy: false,
            available: None,
            last_check: None,
        }
    }
}
pub fn check() -> Result<Option<AvailableUpdate>> {
    let client = Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(30))
        .user_agent("KrillDesktop-Updater/2.0")
        .build()?;
    let response = client
        .get(LATEST_URL)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .context("检查更新失败，请检查网络后重试。")?;
    if response.status().as_u16() == 404 {
        return Ok(None);
    }
    ensure!(
        response.status().as_u16() == 200,
        "GitHub 暂时不可用或请求受限，请稍后重试。"
    );
    let mut bytes = Vec::new();
    response.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1024 * 1024, "更新信息过大。");
    let release: Release = serde_json::from_slice(&bytes).context("无法读取更新信息。")?;
    let current: Version = CURRENT_VERSION
        .parse()
        .map_err(|_| anyhow::anyhow!("当前版本无效。"))?;
    let Some(version) = release
        .tag_name
        .parse::<Version>()
        .ok()
        .filter(|v| *v > current && !release.draft && !release.prerelease)
    else {
        return Ok(None);
    };
    let asset = release
        .update(current, PLATFORM)
        .context("新版本尚未提供可校验的当前平台安装包，请稍后重试。")?
        .clone();
    Ok(Some(AvailableUpdate { version, asset }))
}
pub fn destination() -> Result<PathBuf> {
    let executable = std::env::current_exe()?;
    #[cfg(target_os = "macos")]
    {
        let app = executable
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .context("请从打包后的应用执行更新。")?;
        ensure!(
            app.extension().is_some_and(|s| s == "app")
                && app.join("Contents/MacOS/KrillMac") == executable,
            "请从打包后的应用执行更新。"
        );
        ensure!(
            !app.to_string_lossy().contains("/AppTranslocation/") && app.canonicalize()? == app,
            "请先将应用移到可写的应用程序目录，再执行更新。"
        );
        Ok(app.to_path_buf())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(executable
            .parent()
            .context("无法定位当前程序。")?
            .to_path_buf())
    }
}
pub struct PreparedUpdate {
    staging: tempfile::TempDir,
    destination: PathBuf,
}
pub fn download(update: &AvailableUpdate, destination: &Path) -> Result<PreparedUpdate> {
    let expected_name = format!("Krill.Desktop-{}-{PLATFORM}.zip", update.version);
    ensure!(
        update.asset.name == expected_name
            && update.asset.browser_download_url
                == format!(
                    "https://github.com/{}/releases/download/v{}/{}",
                    krill_core::release::REPOSITORY,
                    update.version,
                    expected_name
                )
            && update.asset.sha256().is_some()
            && update.asset.size > 0
            && update.asset.size <= 1_073_741_824,
        "更新安装包来源无效。"
    );
    let parent = if cfg!(target_os = "macos") {
        destination.parent().context("无法定位应用目录。")?
    } else {
        destination
    };
    let staging = tempfile::Builder::new()
        .prefix(".krill-update-")
        .tempdir_in(parent)
        .context("无法准备更新，请确认应用所在目录可写。")?;
    let archive = staging.path().join("download.zip");
    let client = Client::builder()
        .timeout(Duration::from_secs(600))
        .user_agent("KrillDesktop-Updater/2.0")
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                attempt.error("Too many redirects")
            } else if attempt.url().scheme() != "https" {
                attempt.error("Insecure update redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()?;
    let mut response = client
        .get(&update.asset.browser_download_url)
        .send()
        .context("安装包下载失败，请稍后重试。")?;
    ensure!(
        response.status().as_u16() == 200,
        "安装包下载失败，请稍后重试。"
    );
    let mut file = File::create(&archive)?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let count = response.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        ensure!(total <= update.asset.size, "安装包大小不匹配。");
        hash.update(&buffer[..count]);
        file.write_all(&buffer[..count])?;
    }
    ensure!(total == update.asset.size, "安装包下载不完整。");
    ensure!(
        Some(format!("{:x}", hash.finalize())) == update.asset.sha256(),
        "安装包 SHA-256 校验失败。"
    );
    file.sync_all()?;
    drop(file);
    let payload = if cfg!(target_os = "macos") {
        staging.path().to_path_buf()
    } else {
        staging.path().join("payload")
    };
    extract_archive(&archive, &payload)?;
    validate_payload(&payload, update.version)?;
    fs::remove_file(archive)?;
    Ok(PreparedUpdate {
        staging,
        destination: destination.to_path_buf(),
    })
}
fn extract_archive(archive: &Path, destination: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(archive)?)?;
    ensure!(zip.len() < 4096, "安装包文件过多。");
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let enclosed = entry
            .enclosed_name()
            .context("安装包路径无效。")?
            .to_path_buf();
        ensure!(!entry.name().contains('\\'), "安装包路径无效。");
        ensure!(
            entry.unix_mode().is_none_or(|m| m & 0o170000 != 0o120000),
            "安装包不允许符号链接。"
        );
        total = total.checked_add(entry.size()).context("安装包过大。")?;
        ensure!(total <= 1_073_741_824, "安装包解压后过大。");
        let target = destination.join(enclosed);
        if entry.is_dir() {
            fs::create_dir_all(target)?;
        } else {
            fs::create_dir_all(target.parent().context("安装包路径无效。")?)?;
            let mut file = File::options().write(true).create_new(true).open(&target)?;
            let expected = entry.size();
            let copied = std::io::copy(&mut (&mut entry).take(expected + 1), &mut file)?;
            ensure!(copied == expected, "安装包文件大小不匹配。");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(
                    &target,
                    fs::Permissions::from_mode(entry.unix_mode().unwrap_or(0o644) & 0o777),
                )?;
            }
        }
    }
    Ok(())
}
fn validate_payload(payload: &Path, version: Version) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let app = payload.join("Krill Desktop.app");
        let plist = app.join("Contents/Info.plist");
        for (key, expected) in [
            ("CFBundleIdentifier", "com.krill.desktop.macos".to_string()),
            ("CFBundleShortVersionString", version.to_string()),
            ("CFBundleExecutable", "KrillMac".to_string()),
        ] {
            let output = Command::new("/usr/libexec/PlistBuddy")
                .args(["-c", &format!("Print :{key}")])
                .arg(&plist)
                .output()?;
            ensure!(
                output.status.success() && String::from_utf8(output.stdout)?.trim() == expected,
                "安装包中的应用或版本不匹配。"
            );
        }
        ensure!(
            app.join("Contents/MacOS/KrillMac").is_file(),
            "安装包缺少可执行文件。"
        );
        for (program, args) in [
            ("/usr/bin/codesign", vec!["--verify", "--strict"]),
            ("/usr/bin/lipo", vec!["-verify_arch", "arm64"]),
        ] {
            let target = if program.ends_with("lipo") {
                app.join("Contents/MacOS/KrillMac")
            } else {
                app.clone()
            };
            let mut command = Command::new(program);
            if program.ends_with("lipo") {
                command.arg(target).args(args);
            } else {
                command.args(args).arg(target);
            }
            let output = command.output()?;
            ensure!(output.status.success(), "安装包校验失败（{}）。", program);
        }
    }
    #[cfg(target_os = "windows")]
    {
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(payload.join("krill-package.json"))?)?;
        ensure!(
            manifest["version"] == version.to_string()
                && manifest["platform"] == PLATFORM
                && manifest["executable"] == "Krill.Desktop.exe"
                && payload.join("Krill.Desktop.exe").is_file(),
            "安装包中的应用或版本不匹配。"
        );
    }
    Ok(())
}
impl PreparedUpdate {
    pub fn install(self) -> Result<()> {
        #[cfg(target_os = "macos")]
        let mut command = {
            let script = self.staging.path().join("install-update.sh");
            fs::write(
                &script,
                include_str!("../../../resources/update/install-update.sh"),
            )?;
            let mut c = Command::new("/bin/bash");
            c.arg(script)
                .arg(std::process::id().to_string())
                .arg(&self.destination)
                .arg(self.staging.path());
            c
        };
        #[cfg(target_os = "windows")]
        let mut command = {
            use std::os::windows::process::CommandExt;
            let config = self.staging.path().join("update.json");
            fs::write(
                &config,
                serde_json::to_vec(
                    &serde_json::json!({"ProcessId":std::process::id(),"Destination":self.destination,"Payload":self.staging.path().join("payload"),"Arguments":std::env::args().skip(1).collect::<Vec<_>>()}),
                )?,
            )?;
            let script = self.staging.path().join("install-update.ps1");
            fs::write(
                &script,
                include_str!("../../../resources/update/install-update.ps1"),
            )?;
            let mut c = Command::new("powershell.exe");
            c.args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(script)
            .arg("-Configuration")
            .arg(config)
            .creation_flags(0x08000000);
            c
        };
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            anyhow::bail!("当前平台不支持安装更新。");
        }
        #[cfg(target_os = "macos")]
        command.args(std::env::args().skip(1));
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("无法启动更新安装组件。")?;
        let _ = self.staging.keep();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zip_extraction_rejects_traversal_and_path_aliases() {
        for names in [
            vec!["../outside"],
            vec!["..\\outside"],
            vec!["same", "nested/../same"],
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("payload.zip");
            let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
            for name in names {
                zip.start_file(name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(b"synthetic").unwrap();
            }
            zip.finish().unwrap();
            assert!(extract_archive(&path, &dir.path().join("payload")).is_err());
            assert!(!dir.path().join("outside").exists());
        }
    }
    #[test]
    fn app_identity_and_version_are_validated_before_replacement() {
        let dir = tempfile::tempdir().unwrap();
        assert!(validate_payload(dir.path(), Version(1, 2, 3)).is_err());
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn signed_arm_app_with_correct_identity_and_version_passes_validation() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("Krill Desktop.app");
        fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        // CI may run tests on Intel; construct an actual arm64 payload rather
        // than assuming the host architecture matches the distribution target.
        let source = dir.path().join("noop.c");
        fs::write(&source, "int main(void) { return 0; }\n").unwrap();
        assert!(
            Command::new("/usr/bin/clang")
                .args(["-arch", "arm64"])
                .arg(source)
                .arg("-o")
                .arg(app.join("Contents/MacOS/KrillMac"))
                .status()
                .unwrap()
                .success()
        );
        fs::write(app.join("Contents/Info.plist"),r#"<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>com.krill.desktop.macos</string><key>CFBundleExecutable</key><string>KrillMac</string><key>CFBundleShortVersionString</key><string>1.2.3</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>"#).unwrap();
        assert!(
            Command::new("/usr/bin/codesign")
                .args(["--force", "--sign", "-"])
                .arg(&app)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
        );
        validate_payload(dir.path(), Version(1, 2, 3)).unwrap();
        assert!(validate_payload(dir.path(), Version(1, 2, 4)).is_err());
    }
}
