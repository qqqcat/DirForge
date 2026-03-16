use dirforge_core::RiskLevel;
use dirforge_platform::move_to_recycle_bin;
use dirforge_telemetry as telemetry;
use serde::Serialize;
use std::io;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DeletionItem {
    pub path: String,
    pub size: u64,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone)]
pub struct DeletionPlan {
    pub files: Vec<DeletionItem>,
    pub reclaimable_bytes: u64,
    pub high_risk_count: usize,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ActionFailureKind {
    Missing,
    PermissionDenied,
    UnsupportedType,
    Protected,
    Io,
    PrecheckMismatch,
}

#[derive(Debug, Clone, Copy)]
struct TargetMeta {
    is_dir: bool,
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
    let mut high = 0usize;
    let mut reclaimable_bytes = 0u64;
    let mut out = Vec::new();
    for (path, size, risk) in files {
        reclaimable_bytes = reclaimable_bytes.saturating_add(size);
        if risk == RiskLevel::High {
            high += 1;
        }
        out.push(DeletionItem { path, size, risk });
    }
    DeletionPlan {
        files: out,
        reclaimable_bytes,
        high_risk_count: high,
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

        let (success, message, failure_kind, retries) = match validation {
            Err(kind) => (false, format!("validation failed: {kind:?}"), Some(kind), 0),
            Ok(meta) => {
                if config.compare_with_dry_run && dry_run_check.is_some() {
                    (
                        false,
                        "validation mismatch with dry-run".to_string(),
                        Some(ActionFailureKind::PrecheckMismatch),
                        0,
                    )
                } else if simulated {
                    (true, format!("simulated {:?}", mode), None, 0)
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
        });
        telemetry::record_action_audit(audit_payload.to_string());

        items.push(ExecutionResultItem {
            path: file.path.clone(),
            success,
            message,
            failure_kind,
            retries,
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
) -> (bool, String, Option<ActionFailureKind>, u8) {
    let mut attempt = 0u8;
    loop {
        let result = match (mode, meta.is_dir) {
            (ExecutionMode::Permanent, false) => std::fs::remove_file(path)
                .map(|_| ())
                .map_err(|e| map_io_error(&e)),
            (ExecutionMode::Permanent, true) => std::fs::remove_dir_all(path)
                .map(|_| ())
                .map_err(|e| map_io_error(&e)),
            (ExecutionMode::RecycleBin, _) => {
                move_to_recycle_bin(path).map_err(|_| ActionFailureKind::Io)
            }
        };

        match result {
            Ok(_) => {
                return (true, "execute ok".to_string(), None, attempt);
            }
            Err(kind) => {
                if attempt >= retries || !matches!(kind, ActionFailureKind::Io) {
                    return (
                        false,
                        format!("execute failed after {} attempt(s): {kind:?}", attempt + 1),
                        Some(kind),
                        attempt,
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
    if mode == ExecutionMode::Permanent && risk == RiskLevel::High {
        return Err(ActionFailureKind::Protected);
    }

    if simulated {
        return Ok(TargetMeta { is_dir: false });
    }

    if !path.exists() {
        return Err(ActionFailureKind::Missing);
    }

    let meta = std::fs::metadata(path).map_err(|e| map_io_error(&e))?;
    if !(meta.is_file() || meta.is_dir()) {
        return Err(ActionFailureKind::UnsupportedType);
    }

    let parent = path.parent().unwrap_or(path);
    if std::fs::OpenOptions::new().read(true).open(parent).is_err() {
        return Err(ActionFailureKind::PermissionDenied);
    }

    Ok(TargetMeta {
        is_dir: meta.is_dir(),
    })
}

fn map_io_error(e: &io::Error) -> ActionFailureKind {
    match e.kind() {
        io::ErrorKind::NotFound => ActionFailureKind::Missing,
        io::ErrorKind::PermissionDenied => ActionFailureKind::PermissionDenied,
        _ => ActionFailureKind::Io,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_and_simulated_execution_smoke() {
        let plan = build_deletion_plan(vec![
            ("a".to_string(), 10, RiskLevel::Low),
            ("b".to_string(), 20, RiskLevel::High),
        ]);
        assert_eq!(plan.reclaimable_bytes, 30);
        assert_eq!(plan.high_risk_count, 1);

        let report = execute_plan_simulated(&plan, ExecutionMode::Permanent);
        assert_eq!(report.attempted, 2);
        assert_eq!(report.failed, 1);
    }
}
