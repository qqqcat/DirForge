use crate::error::{map_io_error, PlatformError, PlatformErrorKind};
use crate::volume::{list_volumes, volume_info};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const STAGING_DIR_NAME: &str = ".dirotter-staging";

static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn stage_for_fast_cleanup(path: &str) -> Result<String, PlatformError> {
    let source = PathBuf::from(path);
    if !source.exists() {
        return Err(PlatformError::new(
            PlatformErrorKind::InvalidInput,
            format!("path does not exist: {path}"),
        ));
    }

    let canonical = source.canonicalize().unwrap_or_else(|_| source.clone());
    let mut last_error = None;
    for staging_root in staging_root_candidates(path, &source)? {
        let staged = staging_root.join(unique_stage_name(&canonical));
        match std::fs::rename(&source, &staged) {
            Ok(_) => return Ok(staged.display().to_string()),
            Err(err) => {
                last_error = Some(PlatformError::new(map_io_error(&err), err.to_string()));
            }
        }
    }

    if let Some(err) = last_error {
        if matches!(
            err.kind,
            PlatformErrorKind::Permission | PlatformErrorKind::Io
        ) {
            fast_delete_source_immediately(&source)?;
            return Ok(source.display().to_string());
        }
        return Err(err);
    }

    Err(PlatformError::new(
        PlatformErrorKind::Io,
        format!("failed to stage path: {path}"),
    ))
}

pub fn purge_staged_path(path: &str) -> Result<(), PlatformError> {
    let target = PathBuf::from(path);
    if !target.exists() {
        return Ok(());
    }
    let metadata = std::fs::metadata(&target)
        .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
    if metadata.is_dir() {
        std::fs::remove_dir_all(&target)
            .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
        return Ok(());
    }

    fast_permanent_delete_file(&target).or_else(|_| {
        std::fs::remove_file(&target)
            .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))
    })
}

pub fn purge_all_staging_roots() -> Result<(), PlatformError> {
    for volume in list_volumes()? {
        let staging_root = PathBuf::from(&volume.mount_point).join(STAGING_DIR_NAME);
        if !staging_root.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(&staging_root) {
            for entry in entries.flatten() {
                let staged = entry.path();
                let _ = purge_staged_path(&staged.display().to_string());
            }
        }
    }
    Ok(())
}

fn staging_root_candidates(path: &str, source: &Path) -> Result<Vec<PathBuf>, PlatformError> {
    let volume = volume_info(path)?;
    let preferred_root = PathBuf::from(volume.mount_point).join(STAGING_DIR_NAME);
    let fallback_root = source.parent().unwrap_or(source).join(STAGING_DIR_NAME);

    let mut roots = Vec::new();
    match create_staging_root(&preferred_root) {
        Ok(root) => roots.push(root),
        Err(err)
            if !matches!(
                err.kind,
                PlatformErrorKind::Permission | PlatformErrorKind::Io
            ) =>
        {
            return Err(err);
        }
        Err(_) => {}
    }

    if fallback_root != preferred_root {
        match create_staging_root(&fallback_root) {
            Ok(root) => roots.push(root),
            Err(err)
                if !matches!(
                    err.kind,
                    PlatformErrorKind::Permission | PlatformErrorKind::Io
                ) =>
            {
                return Err(err);
            }
            Err(_) => {}
        }
    }

    if roots.is_empty() {
        return Err(PlatformError::new(
            PlatformErrorKind::Permission,
            format!("no writable staging root available for {path}"),
        ));
    }

    Ok(roots)
}

fn create_staging_root(root: &Path) -> Result<PathBuf, PlatformError> {
    std::fs::create_dir_all(root)
        .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
    hide_staging_root(root);
    Ok(root.to_path_buf())
}

fn fast_delete_source_immediately(source: &Path) -> Result<(), PlatformError> {
    let metadata = std::fs::metadata(source)
        .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
    if metadata.is_dir() {
        std::fs::remove_dir_all(source)
            .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
        return Ok(());
    }

    fast_permanent_delete_file(source).or_else(|_| {
        std::fs::remove_file(source)
            .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))
    })
}

fn unique_stage_name(path: &Path) -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let pid = std::process::id();
    let seq = STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
    let stem = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("item")
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>();
    format!("{stamp}-{pid}-{seq}-{stem}")
}

#[cfg(target_os = "windows")]
fn hide_staging_root(root: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN};

    let wide: Vec<u16> = root.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let _ = SetFileAttributesW(wide.as_ptr(), FILE_ATTRIBUTE_HIDDEN);
    }
}

#[cfg(not(target_os = "windows"))]
fn hide_staging_root(_root: &Path) {}

#[cfg(target_os = "windows")]
fn fast_permanent_delete_file(path: &Path) -> Result<(), PlatformError> {
    use std::mem::size_of;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FileDispositionInfoEx, SetFileInformationByHandle, DELETE,
        FILE_DISPOSITION_FLAG_DELETE, FILE_DISPOSITION_FLAG_IGNORE_READONLY_ATTRIBUTE,
        FILE_DISPOSITION_INFO_EX, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            DELETE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        return Err(last_platform_error("CreateFileW"));
    }

    let info = FILE_DISPOSITION_INFO_EX {
        Flags: FILE_DISPOSITION_FLAG_DELETE | FILE_DISPOSITION_FLAG_IGNORE_READONLY_ATTRIBUTE,
    };
    let status = unsafe {
        SetFileInformationByHandle(
            handle,
            FileDispositionInfoEx,
            &info as *const _ as *const _,
            size_of::<FILE_DISPOSITION_INFO_EX>() as u32,
        )
    };
    unsafe {
        let _ = CloseHandle(handle);
    }

    if status == 0 {
        return Err(last_platform_error("SetFileInformationByHandle"));
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn fast_permanent_delete_file(path: &Path) -> Result<(), PlatformError> {
    std::fs::remove_file(path).map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))
}

#[cfg(target_os = "windows")]
fn last_platform_error(op: &str) -> PlatformError {
    use windows_sys::Win32::Foundation::{
        GetLastError, ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_SHARING_VIOLATION,
    };

    let code = unsafe { GetLastError() };
    let kind = match code {
        ERROR_FILE_NOT_FOUND => PlatformErrorKind::NotFound,
        ERROR_ACCESS_DENIED => PlatformErrorKind::Permission,
        ERROR_SHARING_VIOLATION => PlatformErrorKind::Busy,
        _ => PlatformErrorKind::Io,
    };
    PlatformError::new(kind, format!("{op} failed with win32 error {code}"))
}

#[cfg(not(target_os = "windows"))]
fn last_platform_error(_op: &str) -> PlatformError {
    PlatformError::new(PlatformErrorKind::Io, "platform error".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_and_purge_file_roundtrip() {
        let root = std::env::temp_dir().join(format!(
            "dirotter_platform_stage_test_{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&root);
        let file = root.join("cache.bin");
        std::fs::write(&file, vec![7u8; 1024]).expect("write");

        let staged = stage_for_fast_cleanup(&file.display().to_string()).expect("stage");
        assert!(!file.exists());
        assert!(Path::new(&staged).exists());

        purge_staged_path(&staged).expect("purge");
        assert!(!Path::new(&staged).exists());

        let _ = std::fs::remove_dir_all(&root);
    }
}
