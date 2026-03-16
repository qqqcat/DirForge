#[derive(Debug, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn reveal_returns_unsupported_on_non_windows() {
        assert_eq!(
            reveal_in_explorer("/tmp/x"),
            Err(PlatformError::Unsupported)
        );
    }
}
