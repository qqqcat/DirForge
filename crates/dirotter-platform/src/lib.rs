mod delete;
mod error;
mod explorer;
mod fs_meta;
mod trash;
mod volume;

use serde::Serialize;

pub use delete::{
    purge_all_staging_roots, purge_staged_path, stage_for_fast_cleanup, STAGING_DIR_NAME,
};
pub use error::{map_io_error, PlatformError, PlatformErrorKind};
pub use explorer::{reveal_in_explorer, select_in_explorer};
pub use fs_meta::{is_reparse_point, normalize_path, stable_file_identity, FileIdentity};
pub use trash::move_to_recycle_bin;
pub use volume::{list_volumes, volume_info, VolumeInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformCapabilities {
    pub reveal_in_explorer: bool,
    pub select_in_explorer: bool,
    pub recycle_bin: bool,
    pub stable_file_identity: bool,
    pub volume_info: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionBoundary {
    Allowed,
    ReadOnly,
    Denied,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathAccessAssessment {
    pub normalized_path: String,
    pub is_dir: bool,
    pub is_reparse_point: bool,
    pub boundary: PermissionBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProcessMemoryStats {
    pub working_set_bytes: u64,
    pub pagefile_bytes: u64,
    pub private_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SystemMemoryStats {
    pub memory_load_percent: u32,
    pub total_phys_bytes: u64,
    pub available_phys_bytes: u64,
    pub low_memory_signal: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SystemMemoryReleaseReport {
    pub before_available_phys_bytes: u64,
    pub after_available_phys_bytes: u64,
    pub before_memory_load_percent: u32,
    pub after_memory_load_percent: u32,
    pub trimmed_current_process: bool,
    pub trimmed_process_count: u32,
    pub scanned_process_count: u32,
    pub trimmed_system_file_cache: bool,
}

impl SystemMemoryReleaseReport {
    pub fn available_phys_delta(&self) -> u64 {
        self.after_available_phys_bytes
            .saturating_sub(self.before_available_phys_bytes)
    }
}

pub fn assess_path_access(path: &str) -> Result<PathAccessAssessment, PlatformError> {
    let normalized = normalize_path(path)?;
    let meta = std::fs::metadata(&normalized).map_err(|e| PlatformError {
        kind: map_io_error(&e),
        message: e.to_string(),
    })?;

    let boundary = if !meta.permissions().readonly() {
        PermissionBoundary::Allowed
    } else {
        PermissionBoundary::ReadOnly
    };

    Ok(PathAccessAssessment {
        normalized_path: normalized.clone(),
        is_dir: meta.is_dir(),
        is_reparse_point: is_reparse_point(&normalized).unwrap_or(false),
        boundary,
    })
}

pub fn capabilities() -> PlatformCapabilities {
    #[cfg(target_os = "windows")]
    {
        return PlatformCapabilities {
            reveal_in_explorer: true,
            select_in_explorer: true,
            recycle_bin: true,
            stable_file_identity: true,
            volume_info: true,
        };
    }
    #[cfg(target_os = "macos")]
    {
        return PlatformCapabilities {
            reveal_in_explorer: true,
            select_in_explorer: true,
            recycle_bin: true,
            stable_file_identity: true,
            volume_info: true,
        };
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        PlatformCapabilities {
            reveal_in_explorer: true,
            select_in_explorer: true,
            recycle_bin: true,
            stable_file_identity: cfg!(any(unix, target_os = "windows")),
            volume_info: true,
        }
    }
}

#[cfg(target_os = "windows")]
pub fn trim_process_memory() -> Result<(), PlatformError> {
    use windows_sys::Win32::System::ProcessStatus::EmptyWorkingSet;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let process = unsafe { GetCurrentProcess() };
    let ok = unsafe { EmptyWorkingSet(process) };
    if ok == 0 {
        Err(PlatformError::new(
            PlatformErrorKind::System,
            "EmptyWorkingSet failed",
        ))
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub fn release_system_memory() -> Result<SystemMemoryReleaseReport, PlatformError> {
    let before = system_memory_stats()?;
    let trimmed_current_process = trim_process_memory().is_ok();
    let (scanned_process_count, trimmed_process_count) = trim_interactive_process_working_sets();
    let trimmed_system_file_cache = trim_system_file_cache().unwrap_or(false);
    let after = system_memory_stats()?;
    Ok(SystemMemoryReleaseReport {
        before_available_phys_bytes: before.available_phys_bytes,
        after_available_phys_bytes: after.available_phys_bytes,
        before_memory_load_percent: before.memory_load_percent,
        after_memory_load_percent: after.memory_load_percent,
        trimmed_current_process,
        trimmed_process_count,
        scanned_process_count,
        trimmed_system_file_cache,
    })
}

#[cfg(target_os = "windows")]
pub fn process_memory_stats() -> Result<ProcessMemoryStats, PlatformError> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let process = unsafe { GetCurrentProcess() };
    let mut counters: PROCESS_MEMORY_COUNTERS_EX = unsafe { zeroed() };
    counters.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    let ok = unsafe {
        GetProcessMemoryInfo(
            process,
            (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
            counters.cb,
        )
    };
    if ok == 0 {
        Err(PlatformError::new(
            PlatformErrorKind::System,
            "GetProcessMemoryInfo failed",
        ))
    } else {
        Ok(ProcessMemoryStats {
            working_set_bytes: counters.WorkingSetSize as u64,
            pagefile_bytes: counters.PagefileUsage as u64,
            private_bytes: Some(counters.PrivateUsage as u64),
        })
    }
}

#[cfg(not(target_os = "windows"))]
pub fn process_memory_stats() -> Result<ProcessMemoryStats, PlatformError> {
    Err(PlatformError::new(
        PlatformErrorKind::Unsupported,
        "process memory stats are only implemented on Windows",
    ))
}

#[cfg(target_os = "windows")]
pub fn system_memory_stats() -> Result<SystemMemoryStats, PlatformError> {
    use std::mem::{size_of, zeroed};
    use std::sync::OnceLock;
    use windows_sys::Win32::System::Memory::{
        CreateMemoryResourceNotification, LowMemoryResourceNotification,
        QueryMemoryResourceNotification,
    };
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    static LOW_MEMORY_HANDLE: OnceLock<Option<usize>> = OnceLock::new();

    let mut memory_status: MEMORYSTATUSEX = unsafe { zeroed() };
    memory_status.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    let ok = unsafe { GlobalMemoryStatusEx(&mut memory_status) };
    if ok == 0 {
        return Err(PlatformError::new(
            PlatformErrorKind::System,
            "GlobalMemoryStatusEx failed",
        ));
    }

    let low_memory_signal = LOW_MEMORY_HANDLE
        .get_or_init(|| {
            let handle = unsafe { CreateMemoryResourceNotification(LowMemoryResourceNotification) };
            if handle.is_null() {
                None
            } else {
                Some(handle as usize)
            }
        })
        .and_then(|handle| {
            let mut state = 0;
            let ok = unsafe {
                QueryMemoryResourceNotification(handle as *mut core::ffi::c_void, &mut state)
            };
            if ok == 0 {
                None
            } else {
                Some(state != 0)
            }
        });

    Ok(SystemMemoryStats {
        memory_load_percent: memory_status.dwMemoryLoad,
        total_phys_bytes: memory_status.ullTotalPhys,
        available_phys_bytes: memory_status.ullAvailPhys,
        low_memory_signal,
    })
}

#[cfg(not(target_os = "windows"))]
pub fn system_memory_stats() -> Result<SystemMemoryStats, PlatformError> {
    Err(PlatformError::new(
        PlatformErrorKind::Unsupported,
        "system memory stats are only implemented on Windows",
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn trim_process_memory() -> Result<(), PlatformError> {
    Err(PlatformError::new(
        PlatformErrorKind::Unsupported,
        "process memory trimming is only implemented on Windows",
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn release_system_memory() -> Result<SystemMemoryReleaseReport, PlatformError> {
    Err(PlatformError::new(
        PlatformErrorKind::Unsupported,
        "system memory release is only implemented on Windows",
    ))
}

#[cfg(target_os = "windows")]
const PROCESS_TRIM_THRESHOLD_BYTES: u64 = 64 * 1024 * 1024;
#[cfg(target_os = "windows")]
const PROCESS_TRIM_LIMIT: usize = 12;

#[cfg(target_os = "windows")]
fn trim_interactive_process_working_sets() -> (u32, u32) {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::ProcessStatus::{
        EmptyWorkingSet, EnumProcesses, GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA,
    };

    #[derive(Clone, Copy)]
    struct Candidate {
        pid: u32,
        working_set_bytes: u64,
    }

    let current_pid = unsafe { GetCurrentProcessId() };
    let mut current_session_id = 0;
    let current_session_ok = unsafe { ProcessIdToSessionId(current_pid, &mut current_session_id) };
    if current_session_ok == 0 {
        return (0, 0);
    }

    let mut process_ids = vec![0u32; 1024];
    let ids = loop {
        let mut bytes_needed = 0u32;
        let ok = unsafe {
            EnumProcesses(
                process_ids.as_mut_ptr(),
                (process_ids.len() * size_of::<u32>()) as u32,
                &mut bytes_needed,
            )
        };
        if ok == 0 {
            return (0, 0);
        }
        let process_count = bytes_needed as usize / size_of::<u32>();
        if process_count < process_ids.len() {
            process_ids.truncate(process_count);
            break process_ids;
        }
        process_ids.resize(process_ids.len() * 2, 0);
    };

    let mut candidates = Vec::new();
    for pid in ids
        .into_iter()
        .filter(|pid| *pid != 0 && *pid != current_pid)
    {
        let mut session_id = 0;
        let session_ok = unsafe { ProcessIdToSessionId(pid, &mut session_id) };
        if session_ok == 0 || session_id != current_session_id {
            continue;
        }

        let process = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_QUOTA,
                0,
                pid,
            )
        };
        if process.is_null() {
            continue;
        }

        let mut counters: PROCESS_MEMORY_COUNTERS_EX = unsafe { zeroed() };
        counters.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        let ok = unsafe {
            GetProcessMemoryInfo(
                process,
                (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
                counters.cb,
            )
        };
        unsafe {
            CloseHandle(process);
        }
        if ok == 0 {
            continue;
        }

        let working_set_bytes = counters.WorkingSetSize as u64;
        if working_set_bytes < PROCESS_TRIM_THRESHOLD_BYTES {
            continue;
        }
        candidates.push(Candidate {
            pid,
            working_set_bytes,
        });
    }

    candidates.sort_by(|a, b| {
        b.working_set_bytes
            .cmp(&a.working_set_bytes)
            .then_with(|| a.pid.cmp(&b.pid))
    });
    let scanned_process_count = candidates.len() as u32;
    let mut trimmed_process_count = 0u32;
    for candidate in candidates.into_iter().take(PROCESS_TRIM_LIMIT) {
        let process = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_QUOTA,
                0,
                candidate.pid,
            )
        };
        if process.is_null() {
            continue;
        }
        let ok = unsafe { EmptyWorkingSet(process) };
        unsafe {
            CloseHandle(process);
        }
        if ok != 0 {
            trimmed_process_count = trimmed_process_count.saturating_add(1);
        }
    }

    (scanned_process_count, trimmed_process_count)
}

#[cfg(target_os = "windows")]
fn trim_system_file_cache() -> Result<bool, PlatformError> {
    use std::ptr::null;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
    use windows_sys::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Memory::SetSystemFileCacheSize;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token = std::ptr::null_mut();
    let token_ok = unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
    };
    if token_ok == 0 {
        return Ok(false);
    }

    let privilege_name: Vec<u16> = "SeIncreaseQuotaPrivilege\0".encode_utf16().collect();
    let mut luid = Default::default();
    let privilege_ok = unsafe { LookupPrivilegeValueW(null(), privilege_name.as_ptr(), &mut luid) };
    if privilege_ok == 0 {
        unsafe {
            CloseHandle(token);
        }
        return Ok(false);
    }

    let mut privileges = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: luid,
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };
    unsafe {
        AdjustTokenPrivileges(
            token,
            0,
            &mut privileges,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
    }
    let privilege_error = unsafe { GetLastError() };
    if privilege_error != 0 {
        unsafe {
            CloseHandle(token);
        }
        return Ok(false);
    }

    let ok = unsafe { SetSystemFileCacheSize(usize::MAX, usize::MAX, 0) };
    unsafe {
        CloseHandle(token);
    }
    if ok == 0 {
        Ok(false)
    } else {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_invalid_path_is_error() {
        let res = reveal_in_explorer("/definitely/missing/dirotter/path");
        assert_eq!(
            res.expect_err("must error").kind,
            PlatformErrorKind::InvalidInput
        );
    }

    #[test]
    fn volume_info_current_dir() {
        let cwd = std::env::current_dir().expect("cwd");
        let info = volume_info(&cwd.display().to_string()).expect("volume info");
        assert!(info.total_bytes >= info.available_bytes);
    }

    #[test]
    fn list_volumes_returns_entries() {
        let volumes = list_volumes().expect("list volumes");
        assert!(!volumes.is_empty());
        assert!(volumes.iter().all(|volume| !volume.mount_point.is_empty()));
    }

    #[test]
    fn normalize_path_current_dir() {
        let cwd = std::env::current_dir().expect("cwd");
        let normalized = normalize_path(&cwd.display().to_string()).expect("normalize");
        assert!(!normalized.is_empty());
    }

    #[test]
    fn capabilities_have_core_features() {
        let c = capabilities();
        assert!(c.reveal_in_explorer);
        assert!(c.volume_info);
    }

    #[test]
    fn assess_path_access_current_dir() {
        let cwd = std::env::current_dir().expect("cwd");
        let assessment = assess_path_access(&cwd.display().to_string()).expect("assessment");
        assert!(assessment.is_dir);
        assert!(!assessment.normalized_path.is_empty());
    }

    #[test]
    fn assess_path_access_missing_path_error() {
        let missing = "/definitely/missing/dirotter/nope";
        let err = assess_path_access(missing).expect_err("must fail");
        assert_eq!(err.kind, PlatformErrorKind::InvalidInput);
    }
}
