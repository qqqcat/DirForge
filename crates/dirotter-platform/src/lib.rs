mod error;
mod explorer;
mod fs_meta;
mod trash;
mod volume;

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
