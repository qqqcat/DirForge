use crate::error::{PlatformError, PlatformErrorKind};
use std::path::PathBuf;

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
