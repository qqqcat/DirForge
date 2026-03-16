use crate::error::{map_io_error, PlatformError, PlatformErrorKind};
use std::path::Path;

fn require_existing(path: &str) -> Result<(), PlatformError> {
    if Path::new(path).exists() {
        Ok(())
    } else {
        Err(PlatformError::new(
            PlatformErrorKind::InvalidInput,
            format!("path does not exist: {path}"),
        ))
    }
}

pub fn reveal_in_explorer(path: &str) -> Result<(), PlatformError> {
    require_existing(path)?;

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
        Ok(())
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
        Ok(())
    }
}

pub fn select_in_explorer(path: &str) -> Result<(), PlatformError> {
    let p = Path::new(path);
    require_existing(path)?;

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", p.display()))
            .spawn()
            .map_err(|e| PlatformError::new(map_io_error(&e), e.to_string()))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let parent = p.parent().unwrap_or_else(|| Path::new("."));
        reveal_in_explorer(&parent.display().to_string())
    }
}
