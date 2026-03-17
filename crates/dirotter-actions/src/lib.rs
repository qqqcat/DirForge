use dirotter_core::RiskLevel;
use dirotter_platform::{move_to_recycle_bin, PlatformErrorKind};
use dirotter_telemetry as telemetry;
use serde::Serialize;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DeletionItem {
    pub path: String,
    pub size: u64,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SelectionOrigin {
    Duplicates,
    TopFiles,
    Manual,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationWarning {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct DeletionPlan {
    pub files: Vec<DeletionItem>,
    pub reclaimable_bytes: u64,
    pub high_risk_count: usize,
    pub protected_count: usize,
    pub dir_count: usize,
    pub file_count: usize,
    pub risk_breakdown: BTreeMap<String, usize>,
    pub validation_warnings: Vec<ValidationWarning>,
    pub selection_origin: SelectionOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    RecycleBin,
    Permanent,
}

#[derive(Debug, Clone)]
pub struct ExecutionResultItem {
    pub path: String,
    pub success: bool,
    pub message: String,
    pub failure_kind: Option<ActionFailureKind>,
    pub retries: u8,
    pub platform_kind: Option<String>,
    pub io_kind: Option<String>,
    pub path_kind: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ActionFailureKind {
    Missing,
    PermissionDenied,
    UnsupportedType,
    Protected,
    NotSupported,
    PlatformUnavailable,
    Io,
    PrecheckMismatch,
}

#[derive(Debug, Clone, Copy)]
struct TargetMeta {
    exists: bool,
    is_dir: bool,
    writable: bool,
    protected_reason: Option<ActionFailureKind>,
}

#[derive(Debug, Clone)]
pub struct ExecutionReport {
    pub mode: ExecutionMode,
    pub attempted: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub items: Vec<ExecutionResultItem>,
}

#[derive(Debug, Clone, Copy)]
pub struct ExecutionConfig {
    pub retries: u8,
    pub compare_with_dry_run: bool,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            retries: 2,
            compare_with_dry_run: true,
        }
    }
}

pub fn build_deletion_plan(files: Vec<(String, u64, RiskLevel)>) -> DeletionPlan {
    build_deletion_plan_with_origin(files, SelectionOrigin::Manual)
}

pub fn build_deletion_plan_with_origin(
    files: Vec<(String, u64, RiskLevel)>,
    selection_origin: SelectionOrigin,
) -> DeletionPlan {
    let mut high = 0usize;
    let mut reclaimable_bytes = 0u64;
    let mut out = Vec::new();
    let mut protected_count = 0usize;
    let mut dir_count = 0usize;
    let mut file_count = 0usize;
    let mut risk_breakdown: BTreeMap<String, usize> = BTreeMap::new();
    let mut validation_warnings = Vec::new();
    for (path, size, risk) in files {
        reclaimable_bytes = reclaimable_bytes.saturating_add(size);
        if risk == RiskLevel::High {
            high += 1;
            protected_count += 1;
        }

        let risk_key = format!("{:?}", risk);
        *risk_breakdown.entry(risk_key).or_insert(0) += 1;

        let kind = path_kind(&path);
        if kind == "dir_like" {
            dir_count += 1;
        } else {
            file_count += 1;
        }

        if size == 0 {
            validation_warnings.push(ValidationWarning {
                path: path.clone(),
                message: "item has zero-size reclaim estimate".to_string(),
            });
        }

        out.push(DeletionItem { path, size, risk });
    }
    DeletionPlan {
        files: out,
        reclaimable_bytes,
        high_risk_count: high,
        protected_count,
        dir_count,
        file_count,
        risk_breakdown,
        validation_warnings,
        selection_origin,
    }
}

pub fn execute_plan_simulated(plan: &DeletionPlan, mode: ExecutionMode) -> ExecutionReport {
    execute_internal(plan, mode, true, ExecutionConfig::default())
}

pub fn execute_plan(plan: &DeletionPlan, mode: ExecutionMode) -> ExecutionReport {
    execute_plan_with_config(plan, mode, ExecutionConfig::default())
}

pub fn execute_plan_with_config(
    plan: &DeletionPlan,
    mode: ExecutionMode,
    config: ExecutionConfig,
) -> ExecutionReport {
    execute_internal(plan, mode, false, config)
}

fn execute_internal(
    plan: &DeletionPlan,
    mode: ExecutionMode,
    simulated: bool,
    config: ExecutionConfig,
) -> ExecutionReport {
    let mut items = Vec::with_capacity(plan.files.len());
    let mut succeeded = 0usize;
    let mut failed = 0usize;

    for file in &plan.files {
        let dry_run_check =
            validate_deletion_target(Path::new(&file.path), file.risk, mode, true).err();
        let validation =
            validate_deletion_target(Path::new(&file.path), file.risk, mode, simulated);

        let (success, message, failure_kind, retries, platform_kind, io_kind, path_kind) =
            match validation {
                Err(kind) => (
                    false,
                    format!("validation failed: {kind:?}"),
                    Some(kind),
                    0,
                    None,
                    None,
                    Some(path_kind(&file.path).to_string()),
                ),
                Ok(meta) => {
                    if config.compare_with_dry_run && dry_run_check.is_some() {
                        (
                            false,
                            "validation mismatch with dry-run".to_string(),
                            Some(ActionFailureKind::PrecheckMismatch),
                            0,
                            None,
                            None,
                            Some(path_kind(&file.path).to_string()),
                        )
                    } else if simulated {
                        (
                            true,
                            format!("simulated {:?}", mode),
                            None,
                            0,
                            None,
                            None,
                            Some(path_kind(&file.path).to_string()),
                        )
                    } else {
                        execute_with_retry(&file.path, mode, meta, config.retries)
                    }
                }
            };

        telemetry::record_action_result(success);
        if success {
            succeeded += 1;
        } else {
            failed += 1;
        }

        let audit_payload = serde_json::json!({
            "path": file.path,
            "mode": format!("{:?}", mode),
            "simulated": simulated,
            "success": success,
            "failure": failure_kind,
            "retries": retries,
            "message": message,
            "platform_kind": platform_kind,
            "io_kind": io_kind,
            "path_kind": path_kind,
        });
        telemetry::record_action_audit(audit_payload.to_string());

        items.push(ExecutionResultItem {
            path: file.path.clone(),
            success,
            message,
            failure_kind,
            retries,
            platform_kind,
            io_kind,
            path_kind,
        });
    }

    ExecutionReport {
        mode,
        attempted: plan.files.len(),
        succeeded,
        failed,
        items,
    }
}

fn execute_with_retry(
    path: &str,
    mode: ExecutionMode,
    meta: TargetMeta,
    retries: u8,
) -> (
    bool,
    String,
    Option<ActionFailureKind>,
    u8,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let mut attempt = 0u8;
    let _ = (meta.exists, meta.writable, meta.protected_reason);
    loop {
        let result = match (mode, meta.is_dir) {
            (ExecutionMode::Permanent, false) => std::fs::remove_file(path)
                .map(|_| ())
                .map_err(|e| map_io_error(&e)),
            (ExecutionMode::Permanent, true) => std::fs::remove_dir_all(path)
                .map(|_| ())
                .map_err(|e| map_io_error(&e)),
            (ExecutionMode::RecycleBin, _) => move_to_recycle_bin(path).map_err(map_platform_error),
        };

        match result {
            Ok(_) => {
                return (
                    true,
                    "execute ok".to_string(),
                    None,
                    attempt,
                    None,
                    None,
                    None,
                );
            }
            Err(kind) => {
                if attempt >= retries || !matches!(kind, ActionFailureKind::Io) {
                    let (platform_kind, io_kind) = failure_dimensions(kind, mode);
                    return (
                        false,
                        format!("execute failed after {} attempt(s): {kind:?}", attempt + 1),
                        Some(kind),
                        attempt,
                        platform_kind,
                        io_kind,
                        Some(path_kind(path).to_string()),
                    );
                }
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(10 * attempt as u64));
            }
        }
    }
}

fn validate_deletion_target(
    path: &Path,
    risk: RiskLevel,
    mode: ExecutionMode,
    simulated: bool,
) -> Result<TargetMeta, ActionFailureKind> {
    let protected_reason = (mode == ExecutionMode::Permanent && risk == RiskLevel::High)
        .then_some(ActionFailureKind::Protected);

    if !path.exists() {
        return Err(ActionFailureKind::Missing);
    }

    let meta = std::fs::metadata(path).map_err(|e| map_io_error(&e))?;
    if !(meta.is_file() || meta.is_dir()) {
        return Err(ActionFailureKind::UnsupportedType);
    }

    let parent = path.parent().unwrap_or(path);
    std::fs::metadata(parent).map_err(|e| map_io_error(&e))?;

    let writable = !meta.permissions().readonly();

    if let Some(kind) = protected_reason {
        return Err(kind);
    }

    if simulated {
        // dry-run keeps real precheck behavior but skips execution upstream
    }

    Ok(TargetMeta {
        exists: true,
        is_dir: meta.is_dir(),
        writable,
        protected_reason,
    })
}

fn map_io_error(e: &io::Error) -> ActionFailureKind {
    match e.kind() {
        io::ErrorKind::NotFound => ActionFailureKind::Missing,
        io::ErrorKind::PermissionDenied => ActionFailureKind::PermissionDenied,
        _ => ActionFailureKind::Io,
    }
}

fn failure_dimensions(
    kind: ActionFailureKind,
    mode: ExecutionMode,
) -> (Option<String>, Option<String>) {
    let platform_kind = if mode == ExecutionMode::RecycleBin {
        Some(format!("{:?}", kind))
    } else {
        None
    };

    let io_kind = if matches!(
        kind,
        ActionFailureKind::Io | ActionFailureKind::Missing | ActionFailureKind::PermissionDenied
    ) {
        Some(format!("{:?}", kind))
    } else {
        None
    };

    (platform_kind, io_kind)
}

fn map_platform_error(err: dirotter_platform::PlatformError) -> ActionFailureKind {
    match err.kind {
        PlatformErrorKind::Unsupported => ActionFailureKind::NotSupported,
        PlatformErrorKind::Permission => ActionFailureKind::PermissionDenied,
        PlatformErrorKind::NotFound | PlatformErrorKind::InvalidInput => ActionFailureKind::Missing,
        PlatformErrorKind::PathNormalization => ActionFailureKind::UnsupportedType,
        PlatformErrorKind::Busy | PlatformErrorKind::Timeout | PlatformErrorKind::Io => {
            ActionFailureKind::Io
        }
        PlatformErrorKind::System => ActionFailureKind::PlatformUnavailable,
    }
}

fn path_kind(path: &str) -> &'static str {
    let p = Path::new(path);
    if p.extension().is_some() {
        "file_like"
    } else {
        "dir_like"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn plan_and_simulated_execution_smoke() {
        let root = std::env::temp_dir().join(format!("dirotter-actions-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&root);
        let low = root.join("a.txt").display().to_string();
        std::fs::write(&low, b"x").expect("seed file");

        let plan = build_deletion_plan_with_origin(
            vec![
                (low, 10, RiskLevel::Low),
                (root.display().to_string(), 20, RiskLevel::High),
            ],
            SelectionOrigin::Duplicates,
        );
        assert_eq!(plan.reclaimable_bytes, 30);
        assert_eq!(plan.high_risk_count, 1);
        assert_eq!(plan.protected_count, 1);
        assert_eq!(plan.selection_origin, SelectionOrigin::Duplicates);
        assert_eq!(plan.file_count + plan.dir_count, plan.files.len());

        let report = execute_plan_simulated(&plan, ExecutionMode::Permanent);
        assert_eq!(report.attempted, 2);
        assert_eq!(report.failed, 1);
    }

    #[test]
    fn permanent_delete_removes_directory_tree() {
        let root = std::env::temp_dir().join(format!(
            "dirotter-actions-delete-dir-{}",
            std::process::id()
        ));
        let nested = root.join("nested");
        let file = nested.join("payload.txt");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&nested).expect("create nested dir");
        fs::write(&file, b"payload").expect("seed file");

        let plan = build_deletion_plan(vec![(root.display().to_string(), 7, RiskLevel::Low)]);
        let report = execute_plan(&plan, ExecutionMode::Permanent);

        assert_eq!(report.succeeded, 1);
        assert!(!root.exists());
    }

    #[test]
    fn permanent_delete_blocks_high_risk_target() {
        let root =
            std::env::temp_dir().join(format!("dirotter-actions-protected-{}", std::process::id()));
        let _ = fs::create_dir_all(&root);
        let plan = build_deletion_plan(vec![(root.display().to_string(), 1, RiskLevel::High)]);
        let report = execute_plan(&plan, ExecutionMode::Permanent);

        assert_eq!(report.failed, 1);
        assert_eq!(
            report.items[0].failure_kind,
            Some(ActionFailureKind::Protected)
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn permanent_delete_permission_denied_on_readonly_parent_dir() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "dirotter-actions-perm-denied-{}",
            std::process::id()
        ));
        let guarded = root.join("guarded");
        let file = guarded.join("locked.txt");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&guarded).expect("create guarded dir");
        fs::write(&file, b"locked").expect("seed file");

        let mut perms = fs::metadata(&guarded)
            .expect("guarded metadata")
            .permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&guarded, perms).expect("set readonly dir perms");

        let plan = build_deletion_plan(vec![(file.display().to_string(), 6, RiskLevel::Low)]);
        let report = execute_plan(&plan, ExecutionMode::Permanent);

        let mut cleanup_perms = fs::metadata(&guarded)
            .expect("guarded metadata after test")
            .permissions();
        cleanup_perms.set_mode(0o755);
        let _ = fs::set_permissions(&guarded, cleanup_perms);
        let _ = fs::remove_dir_all(&root);

        assert_eq!(report.failed, 1);
        assert_eq!(
            report.items[0].failure_kind,
            Some(ActionFailureKind::PermissionDenied)
        );
    }

    #[cfg(windows)]
    #[test]
    fn permanent_delete_fails_when_file_is_exclusively_locked() {
        use std::os::windows::fs::OpenOptionsExt;

        let root =
            std::env::temp_dir().join(format!("dirotter-actions-locked-{}", std::process::id()));
        let file = root.join("locked.txt");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create root");
        fs::write(&file, b"locked").expect("seed file");

        let handle = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&file)
            .expect("lock file");

        let plan = build_deletion_plan(vec![(file.display().to_string(), 8, RiskLevel::Low)]);
        let report = execute_plan(&plan, ExecutionMode::Permanent);

        drop(handle);
        let _ = fs::remove_file(&file);
        let _ = fs::remove_dir_all(&root);

        assert_eq!(report.failed, 1);
        assert!(
            matches!(
                report.items[0].failure_kind,
                Some(ActionFailureKind::PermissionDenied | ActionFailureKind::Io)
            ),
            "expected locked file to fail with PermissionDenied or Io, got {:?}",
            report.items[0].failure_kind
        );
    }
}
