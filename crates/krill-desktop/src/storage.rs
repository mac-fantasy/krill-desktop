use anyhow::{Context, Result};
use krill_core::{auth::Session, state::Preferences};
use std::{fs, io::Write, path::PathBuf};

#[derive(Clone)]
pub struct Storage {
    pub directory: PathBuf,
    pub demo: bool,
}
impl Storage {
    pub fn new(demo: bool, directory: Option<PathBuf>) -> Result<Self> {
        let directory = if let Some(directory) = directory {
            directory
        } else {
            #[cfg(target_os = "macos")]
            let base = PathBuf::from(std::env::var_os("HOME").context("Missing user directory")?)
                .join("Library/Application Support/KrillDesktop");
            #[cfg(target_os = "windows")]
            let base = PathBuf::from(
                std::env::var_os("LOCALAPPDATA").context("Missing local application directory")?,
            )
            .join("KrillDesktop");
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let base = PathBuf::from(std::env::var_os("HOME").context("Missing user directory")?)
                .join(".local/share/KrillDesktop");
            base.join(if demo { "Demo" } else { "Account" })
        };
        fs::create_dir_all(&directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self { directory, demo })
    }
    pub fn preferences(&self) -> Preferences {
        let mut p = fs::read(self.directory.join("settings.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Preferences>(&b).ok())
            .unwrap_or_default();
        p.normalize();
        p
    }
    pub fn save_preferences(&self, p: &Preferences) -> Result<()> {
        self.write("settings.json", &serde_json::to_vec_pretty(p)?)
    }
    pub fn session(&self) -> Result<Option<Session>> {
        if self.demo {
            return Ok(None);
        }
        #[cfg(target_os = "macos")]
        {
            match security_framework::passwords::get_generic_password(
                "com.krill.desktop.macos.account",
                "krill-session",
            ) {
                Ok(data) => Ok(std::str::from_utf8(&data).ok().and_then(Session::parse)),
                Err(e) if e.code() == -25300 => Ok(None),
                Err(_) => anyhow::bail!("暂时无法读取钥匙串，请解锁后重新连接。"),
            }
        }
        #[cfg(target_os = "windows")]
        {
            let path = self.directory.join("session.dat");
            if !path.exists() {
                return Ok(None);
            }
            let data = fs::read(path)?;
            let plain = windows::unprotect(&data)?;
            Ok(std::str::from_utf8(&plain).ok().and_then(Session::parse))
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Ok(None)
        }
    }
    pub fn save_session(&self, session: &Session) -> Result<()> {
        anyhow::ensure!(!self.demo, "演示模式不保存账号会话。");
        #[cfg(target_os = "macos")]
        security_framework::passwords::set_generic_password(
            "com.krill.desktop.macos.account",
            "krill-session",
            session.expose().as_bytes(),
        )
        .map_err(|_| anyhow::anyhow!("无法保存钥匙串会话。"))?;
        #[cfg(target_os = "windows")]
        self.write(
            "session.dat",
            &windows::protect(session.expose().as_bytes())?,
        )?;
        Ok(())
    }
    pub fn delete_session(&self) -> Result<()> {
        if self.demo {
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        match security_framework::passwords::delete_generic_password(
            "com.krill.desktop.macos.account",
            "krill-session",
        ) {
            Ok(()) => {}
            Err(e) if e.code() == -25300 => {}
            Err(_) => anyhow::bail!("本机钥匙串会话未能清除，请检查钥匙串访问权限。"),
        }
        #[cfg(target_os = "windows")]
        {
            let path = self.directory.join("session.dat");
            if path.exists() {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    }
    fn write(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)?;
        temporary.write_all(bytes)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(self.directory.join(name))
            .map_err(|_| anyhow::anyhow!("无法保存本机数据。"))?;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use ::windows::Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    };
    use anyhow::Result;
    const ENTROPY: &[u8] = b"Krill.Desktop/session/v1";
    fn crypt(data: &[u8], protect: bool) -> Result<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len().try_into()?,
            pbData: data.as_ptr().cast_mut(),
        };
        let entropy = CRYPT_INTEGER_BLOB {
            cbData: ENTROPY.len() as u32,
            pbData: ENTROPY.as_ptr().cast_mut(),
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        unsafe {
            let result = if protect {
                CryptProtectData(
                    &input,
                    None,
                    Some(&entropy),
                    None,
                    None,
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output,
                )
            } else {
                CryptUnprotectData(
                    &input,
                    None,
                    Some(&entropy),
                    None,
                    None,
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output,
                )
            };
            result.map_err(|_| anyhow::anyhow!("无法访问本机加密会话。"))?;
            let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            LocalFree(Some(HLOCAL(output.pbData.cast())));
            Ok(bytes)
        }
    }
    pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
        crypt(data, true)
    }
    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
        crypt(data, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;
    #[test]
    fn demo_storage_isolated_and_preferences_atomic() {
        let directory = tempfile::tempdir().unwrap();
        let storage = Storage::new(true, Some(directory.path().join("demo"))).unwrap();
        let p = Preferences {
            refresh_seconds: 120,
            selected_subscription_id: Some(205),
            ..Default::default()
        };
        storage.save_preferences(&p).unwrap();
        assert_eq!(storage.preferences().refresh_seconds, 120);
        assert_eq!(storage.preferences().selected_subscription_id, Some(205));
        assert!(storage.session().unwrap().is_none());
        assert!(
            storage
                .save_session(&Session::parse("synthetic.payload.signature").unwrap())
                .is_err()
        );
        assert_eq!(fs::read_dir(&storage.directory).unwrap().count(), 1);
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn dpapi_credential_roundtrip_matches_legacy_entropy_and_stays_out_of_preferences() {
        let directory = tempfile::tempdir().unwrap();
        let storage = Storage::new(false, Some(directory.path().join("account"))).unwrap();
        let token = Session::parse("synthetic.header.signature").unwrap();
        storage.save_session(&token).unwrap();
        assert_eq!(storage.session().unwrap().unwrap().expose(), token.expose());
        let encrypted = fs::read(storage.directory.join("session.dat")).unwrap();
        assert!(
            !encrypted
                .windows(token.expose().len())
                .any(|v| v == token.expose().as_bytes())
        );
        storage.save_preferences(&Preferences::default()).unwrap();
        assert!(
            !fs::read_to_string(storage.directory.join("settings.json"))
                .unwrap()
                .contains(token.expose())
        );
        storage.delete_session().unwrap();
        assert!(storage.session().unwrap().is_none());
    }
}
