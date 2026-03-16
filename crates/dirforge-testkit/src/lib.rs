use std::fs;
use std::path::PathBuf;

pub struct FixtureTree {
    pub root: PathBuf,
}

impl FixtureTree {
    pub fn sample() -> std::io::Result<Self> {
        let root = std::env::temp_dir().join(format!("dirforge_fixture_{}", std::process::id()));
        if root.exists() {
            let _ = fs::remove_dir_all(&root);
        }
        fs::create_dir_all(root.join("nested"))?;
        fs::write(root.join("a.bin"), vec![1u8; 16])?;
        fs::write(root.join("nested/b.bin"), vec![2u8; 32])?;
        Ok(Self { root })
    }
}
