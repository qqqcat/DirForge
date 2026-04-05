use crate::error::{PlatformError, PlatformErrorKind};
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::time::Duration;

pub fn move_to_recycle_bin(path: &str) -> Result<(), PlatformError> {
    let p = PathBuf::from(path);
    if !p.exists() {
        return Err(PlatformError::new(
            PlatformErrorKind::InvalidInput,
            format!("path does not exist: {path}"),
        ));
    }
    let original_path = p.canonicalize().unwrap_or_else(|_| p.clone());
    trash::delete(&p).map_err(|e| PlatformError::new(PlatformErrorKind::System, e.to_string()))?;
    verify_recycle_bin_entry(&original_path)?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn verify_recycle_bin_entry(original_path: &std::path::Path) -> Result<(), PlatformError> {
    let original_key = recycle_compare_key(original_path);
    for _ in 0..10 {
        if let Ok(items) = trash::os_limited::list() {
            if items
                .iter()
                .any(|item| recycle_compare_key(&item.original_path()) == original_key)
            {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(150));
    }

    Err(PlatformError::new(
        PlatformErrorKind::System,
        format!(
            "item was deleted but could not be verified in recycle bin: {}",
            original_path.display()
        ),
    ))
}

#[cfg(not(target_os = "windows"))]
fn verify_recycle_bin_entry(_original_path: &std::path::Path) -> Result<(), PlatformError> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn recycle_compare_key(path: &std::path::Path) -> String {
    path.display()
        .to_string()
        .trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .to_lowercase()
}
