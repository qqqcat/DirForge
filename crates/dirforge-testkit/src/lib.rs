use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct FixtureTree {
    pub root: PathBuf,
}

impl FixtureTree {
    pub fn sample() -> std::io::Result<Self> {
        let root = unique_root("sample");
        fs::create_dir_all(root.join("nested"))?;
        fs::write(root.join("a.bin"), vec![1u8; 16])?;
        fs::write(root.join("nested/b.bin"), vec![2u8; 32])?;
        Ok(Self { root })
    }

    pub fn with_symlink() -> std::io::Result<Self> {
        let fixture = Self::sample()?;
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink(fixture.root.join("a.bin"), fixture.root.join("a.link"));
        }
        #[cfg(windows)]
        {
            let _ = std::os::windows::fs::symlink_file(
                fixture.root.join("a.bin"),
                fixture.root.join("a.link"),
            );
        }
        Ok(fixture)
    }

    pub fn restricted_dir() -> std::io::Result<Self> {
        let root = unique_root("restricted");
        fs::create_dir_all(root.join("locked"))?;
        fs::write(root.join("locked/inside.bin"), vec![7u8; 8])?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(root.join("locked"))?.permissions();
            perms.set_mode(0o000);
            fs::set_permissions(root.join("locked"), perms)?;
        }
        Ok(Self { root })
    }
}

impl Drop for FixtureTree {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(self.root.join("locked")) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(self.root.join("locked"), perms);
            }
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn unique_root(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "dirforge_fixture_{}_{}_{}",
        tag,
        std::process::id(),
        nanos
    ))
}
