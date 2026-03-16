use crate::error::{map_io_error, PlatformError, PlatformErrorKind};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeInfo {
    pub mount_point: String,
    pub name: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
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
        .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
    let canonical_key = mount_compare_key(&canonical);

    let mut best: Option<VolumeInfo> = None;
    for d in &disks {
        let mount = d.mount_point();
        let mount_key = mount_compare_key(mount);
        if canonical_key.starts_with(&mount_key) {
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

fn mount_compare_key(path: &std::path::Path) -> String {
    #[cfg(target_os = "windows")]
    {
        path.display()
            .to_string()
            .trim_start_matches(r"\\?\")
            .replace('/', "\\")
            .to_lowercase()
    }

    #[cfg(not(target_os = "windows"))]
    {
        path.display().to_string()
    }
}
