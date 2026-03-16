use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformErrorKind {
    Unsupported,
    InvalidInput,
    Io,
    Permission,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformError {
    pub kind: PlatformErrorKind,
    pub message: String,
}

impl PlatformError {
    fn new(kind: PlatformErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeInfo {
    pub mount_point: String,
    pub name: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

pub fn reveal_in_explorer(path: &str) -> Result<(), PlatformError> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(PlatformError::new(
            PlatformErrorKind::InvalidInput,
            format!("path does not exist: {path}"),
        ));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| PlatformError::new(PlatformErrorKind::Io, e.to_string()))?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| PlatformError::new(PlatformErrorKind::Io, e.to_string()))?;
        Ok(())
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| PlatformError::new(PlatformErrorKind::Io, e.to_string()))?;
        Ok(())
    }
}

pub fn select_in_explorer(path: &str) -> Result<(), PlatformError> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(PlatformError::new(
            PlatformErrorKind::InvalidInput,
            format!("path does not exist: {path}"),
        ));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", p.display()))
            .spawn()
            .map_err(|e| PlatformError::new(PlatformErrorKind::Io, e.to_string()))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let parent = p.parent().unwrap_or_else(|| Path::new("."));
        reveal_in_explorer(&parent.display().to_string())
    }
}

pub fn move_to_recycle_bin(path: &str) -> Result<(), PlatformError> {
    let p = PathBuf::from(path);
    if !p.exists() {
        return Err(PlatformError::new(
            PlatformErrorKind::InvalidInput,
            format!("path does not exist: {path}"),
        ));
    }
    trash::delete(&p).map_err(|e| PlatformError::new(PlatformErrorKind::System, e.to_string()))
}

pub fn is_reparse_point(path: &str) -> Result<bool, PlatformError> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| PlatformError::new(PlatformErrorKind::Io, e.to_string()))?;

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        Ok((meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(meta.file_type().is_symlink())
    }
}

pub fn volume_info(path: &str) -> Result<VolumeInfo, PlatformError> {
    use sysinfo::Disks;

    let p = PathBuf::from(path);
    if !p.exists() {
        return Err(PlatformError::new(
            PlatformErrorKind::InvalidInput,
            format!("path does not exist: {path}"),
        ));
    }

    let disks = Disks::new_with_refreshed_list();
    let canonical = p
        .canonicalize()
        .map_err(|e| PlatformError::new(PlatformErrorKind::Io, e.to_string()))?;

    let mut best: Option<VolumeInfo> = None;
    for d in &disks {
        let mount = d.mount_point();
        if canonical.starts_with(mount) {
            let candidate = VolumeInfo {
                mount_point: mount.display().to_string(),
                name: d.name().to_string_lossy().to_string(),
                total_bytes: d.total_space(),
                available_bytes: d.available_space(),
            };
            if best
                .as_ref()
                .map(|b| candidate.mount_point.len() > b.mount_point.len())
                .unwrap_or(true)
            {
                best = Some(candidate);
            }
        }
    }

    best.ok_or_else(|| PlatformError::new(PlatformErrorKind::Unsupported, "no volume found"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_invalid_path_is_error() {
        let res = reveal_in_explorer("/definitely/missing/dirforge/path");
        assert_eq!(res.expect_err("must error").kind, PlatformErrorKind::InvalidInput);
    }

    #[test]
    fn volume_info_current_dir() {
        let cwd = std::env::current_dir().expect("cwd");
        let info = volume_info(&cwd.display().to_string()).expect("volume info");
        assert!(info.total_bytes >= info.available_bytes);
    }
}
