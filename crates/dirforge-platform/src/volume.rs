use crate::error::{map_io_error, PlatformError, PlatformErrorKind};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeInfo {
    pub mount_point: String,
    pub name: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

pub fn list_volumes() -> Result<Vec<VolumeInfo>, PlatformError> {
    use sysinfo::Disks;

    let mut volumes: Vec<VolumeInfo> = Disks::new_with_refreshed_list()
        .iter()
        .map(|disk| VolumeInfo {
            mount_point: disk.mount_point().display().to_string(),
            name: disk.name().to_string_lossy().to_string(),
            total_bytes: disk.total_space(),
            available_bytes: disk.available_space(),
        })
        .collect();

    volumes.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    Ok(volumes)
}

pub fn volume_info(path: &str) -> Result<VolumeInfo, PlatformError> {
    let p = PathBuf::from(path);
    if !p.exists() {
        return Err(PlatformError::new(
            PlatformErrorKind::InvalidInput,
            format!("path does not exist: {path}"),
        ));
    }

    let disks = list_volumes()?;
    let canonical = p
        .canonicalize()
        .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
    let canonical_key = mount_compare_key(&canonical);

    let mut best: Option<VolumeInfo> = None;
    for d in &disks {
        let mount = PathBuf::from(&d.mount_point);
        let mount_key = mount_compare_key(&mount);
        if canonical_key.starts_with(&mount_key) {
            let candidate = d.clone();
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
