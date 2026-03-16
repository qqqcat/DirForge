use dirforge_core::RiskLevel;
use dirforge_platform::move_to_recycle_bin;
use dirforge_telemetry as telemetry;
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionFailureKind {
    Missing,
    PermissionDenied,
    UnsupportedType,
    Protected,
    Io,
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
    execute_internal(plan, mode, true)
}

pub fn execute_plan(plan: &DeletionPlan, mode: ExecutionMode) -> ExecutionReport {
    execute_internal(plan, mode, false)
}

fn execute_internal(plan: &DeletionPlan, mode: ExecutionMode, simulated: bool) -> ExecutionReport {
    let mut items = Vec::with_capacity(plan.files.len());
    let mut succeeded = 0usize;
    let mut failed = 0usize;

    for file in &plan.files {
        let (success, message, failure_kind) =
            match validate_deletion_target(Path::new(&file.path), file.risk, mode, simulated) {
                Err(kind) => (false, format!("validation failed: {kind:?}"), Some(kind)),
                Ok(meta) if simulated => (true, format!("simulated {:?}", mode), None),
                Ok(meta) => match (mode, meta.is_dir) {
                    (ExecutionMode::Permanent, false) => match std::fs::remove_file(&file.path) {
                        Ok(_) => (true, "permanent delete ok".to_string(), None),
                        Err(e) => (false, format!("delete failed: {e}"), Some(map_io_error(&e))),
                    },
                    (ExecutionMode::Permanent, true) => match std::fs::remove_dir_all(&file.path) {
                        Ok(_) => (true, "permanent delete dir ok".to_string(), None),
                        Err(e) => (false, format!("delete failed: {e}"), Some(map_io_error(&e))),
                    },
                    (ExecutionMode::RecycleBin, _) => match move_to_recycle_bin(&file.path) {
                        Ok(_) => (true, "moved to recycle bin".to_string(), None),
                        Err(e) => (
                            false,
                            format!("recycle failed: {}", e.message),
                            Some(ActionFailureKind::Io),
                        ),
                    },
                },
            };

        telemetry::record_action_result(success);
        if success {
            succeeded += 1;
        } else {
            failed += 1;
        }
        items.push(ExecutionResultItem {
            path: file.path.clone(),
            success,
            message,
            failure_kind,
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
