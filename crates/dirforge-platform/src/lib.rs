#[derive(Debug)]
pub enum PlatformError {
    Unsupported,
    Io,
}

pub fn reveal_in_explorer(path: &str) -> Result<(), PlatformError> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|_| PlatformError::Io)?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Err(PlatformError::Unsupported)
    }
}
