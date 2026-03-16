#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformErrorKind {
    Unsupported,
    InvalidInput,
    NotFound,
    Permission,
    Io,
    PathNormalization,
    Timeout,
    Busy,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformError {
    pub kind: PlatformErrorKind,
    pub message: String,
}

impl PlatformError {
    pub fn new(kind: PlatformErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

pub fn map_io_error(e: &std::io::Error) -> PlatformErrorKind {
    use std::io::ErrorKind;
    match e.kind() {
        ErrorKind::NotFound => PlatformErrorKind::NotFound,
        ErrorKind::PermissionDenied => PlatformErrorKind::Permission,
        ErrorKind::TimedOut => PlatformErrorKind::Timeout,
        ErrorKind::WouldBlock => PlatformErrorKind::Busy,
        ErrorKind::InvalidInput | ErrorKind::InvalidData => PlatformErrorKind::InvalidInput,
        _ => PlatformErrorKind::Io,
    }
}
