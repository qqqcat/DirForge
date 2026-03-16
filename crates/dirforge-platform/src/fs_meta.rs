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
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::metadata(path)
            .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
        Ok(FileIdentity {
            dev: meta.dev(),
            inode: meta.ino(),
        })
    }
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{
            CreateFileW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
            FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
            FILE_SHARE_WRITE, OPEN_EXISTING,
        };

        let wide_path = OsStr::new(path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>();

        let handle = unsafe {
            CreateFileW(
                wide_path.as_ptr(),
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                std::ptr::null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let err = std::io::Error::last_os_error();
            return Err(PlatformError::new(map_io_error(&err), err.to_string()));
        }

        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        let ok = unsafe { GetFileInformationByHandle(handle, &mut info) };
        let close_result = unsafe { CloseHandle(handle) };

        if ok == 0 {
            let err = std::io::Error::last_os_error();
            return Err(PlatformError::new(map_io_error(&err), err.to_string()));
        }

        if close_result == 0 {
            let err = std::io::Error::last_os_error();
            return Err(PlatformError::new(map_io_error(&err), err.to_string()));
        }

        let index = ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64;
        Ok(FileIdentity {
            dev: info.dwVolumeSerialNumber as u64,
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
