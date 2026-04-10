use crate::error::{map_io_error, PlatformError, PlatformErrorKind};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExplorerOpenTarget {
    Reveal,
    Select,
}

fn require_existing(path: &str) -> Result<(), PlatformError> {
    if Path::new(path).exists() {
        Ok(())
    } else {
        Err(PlatformError::new(
            PlatformErrorKind::InvalidInput,
            format!("path does not exist: {path}"),
        ))
    }
}

fn explorer_open_target(path: &Path) -> ExplorerOpenTarget {
    if path.is_dir() {
        ExplorerOpenTarget::Reveal
    } else {
        ExplorerOpenTarget::Select
    }
}

fn normalized_explorer_path(path: &Path) -> Result<String, PlatformError> {
    let canonical = path
        .canonicalize()
        .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
    let mut normalized = canonical.display().to_string().replace('/', "\\");
    if let Some(stripped) = normalized.strip_prefix(r"\\?\") {
        normalized = stripped.to_string();
    }
    Ok(normalized)
}

pub fn reveal_in_explorer(path: &str) -> Result<(), PlatformError> {
    require_existing(path)?;

    #[cfg(target_os = "windows")]
    {
        let normalized = normalized_explorer_path(Path::new(path))?;
        std::process::Command::new("explorer")
            .arg(normalized)
            .spawn()
            .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
        Ok(())
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
        Ok(())
    }
}

pub fn select_in_explorer(path: &str) -> Result<(), PlatformError> {
    let p = Path::new(path);
    require_existing(path)?;

    if explorer_open_target(p) == ExplorerOpenTarget::Reveal {
        return reveal_in_explorer(&p.display().to_string());
    }

    #[cfg(target_os = "windows")]
    {
        let normalized = normalized_explorer_path(p)?;
        use std::os::windows::process::CommandExt;

        std::process::Command::new("explorer.exe")
            .raw_arg(format!(r#"/select,"{}""#, normalized))
            .spawn()
            .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let parent = p.parent().unwrap_or_else(|| Path::new("."));
        reveal_in_explorer(&parent.display().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_path(name: &str) -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "dirotter-explorer-test-{}-{}-{}",
            name,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
                + suffix as u128
        ))
    }

    #[test]
    fn directory_targets_are_revealed_directly() {
        let dir = make_temp_path("dir");
        fs::create_dir_all(&dir).expect("create dir");

        assert_eq!(explorer_open_target(&dir), ExplorerOpenTarget::Reveal);

        fs::remove_dir_all(&dir).expect("cleanup dir");
    }

    #[test]
    fn file_targets_are_selected_from_parent() {
        let dir = make_temp_path("file-parent");
        fs::create_dir_all(&dir).expect("create dir");
        let file = dir.join("sample.txt");
        fs::write(&file, "dir otter").expect("write file");

        assert_eq!(explorer_open_target(&file), ExplorerOpenTarget::Select);

        fs::remove_file(&file).expect("cleanup file");
        fs::remove_dir_all(&dir).expect("cleanup dir");
    }
}
