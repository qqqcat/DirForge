mod error;
mod explorer;
mod fs_meta;
mod trash;
mod volume;

pub use error::{map_io_error, PlatformError, PlatformErrorKind};
pub use explorer::{reveal_in_explorer, select_in_explorer};
pub use fs_meta::{is_reparse_point, normalize_path, stable_file_identity, FileIdentity};
pub use trash::move_to_recycle_bin;
pub use volume::{volume_info, VolumeInfo};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_invalid_path_is_error() {
        let res = reveal_in_explorer("/definitely/missing/dirforge/path");
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
    fn normalize_path_current_dir() {
        let cwd = std::env::current_dir().expect("cwd");
        let normalized = normalize_path(&cwd.display().to_string()).expect("normalize");
        assert!(!normalized.is_empty());
    }
}
