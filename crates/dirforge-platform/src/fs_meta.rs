use crate::error::{map_io_error, PlatformError, PlatformErrorKind};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIdentity {
    pub dev: u64,
    pub inode: u64,
}

pub fn normalize_path(path: &str) -> Result<String, PlatformError> {
    let p = PathBuf::from(path);
    if !p.exists() {
        return Err(PlatformError::new(
            PlatformErrorKind::InvalidInput,
            format!("path does not exist: {path}"),
        ));
    }
    let canonical = p
        .canonicalize()
        .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;

    #[cfg(target_os = "windows")]
    {
        let mut normalized = canonical.display().to_string().replace('/', "\\");
        if !normalized.starts_with(r"\\?\") {
            normalized = format!(r"\\?\{}", normalized);
        }
        Ok(normalized)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(canonical.display().to_string())
    }
}

pub fn is_reparse_point(path: &str) -> Result<bool, PlatformError> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;

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

pub fn stable_file_identity(path: &str) -> Result<FileIdentity, PlatformError> {
    let meta =
        std::fs::metadata(path).map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(FileIdentity {
            dev: meta.dev(),
            inode: meta.ino(),
        })
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        let index = ((meta.file_index_high() as u64) << 32) | meta.file_index_low() as u64;
        Ok(FileIdentity {
            dev: meta.volume_serial_number().unwrap_or_default() as u64,
            inode: index,
        })
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        Err(PlatformError::new(
            PlatformErrorKind::Unsupported,
            "stable_file_identity unsupported on this platform",
        ))
    }
}
