use dirforge_core::RiskLevel;

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
    let mut items = Vec::with_capacity(plan.files.len());
    let mut succeeded = 0usize;
    let mut failed = 0usize;

    for file in &plan.files {
        let (success, message) = match (mode, file.risk) {
            (ExecutionMode::Permanent, RiskLevel::High) => (
                false,
                "blocked: high-risk item requires manual override".to_string(),
            ),
            (ExecutionMode::Permanent, _) => (true, "simulated permanent delete".to_string()),
            (ExecutionMode::RecycleBin, _) => (true, "simulated recycle-bin move".to_string()),
        };

        if success {
            succeeded += 1;
        } else {
            failed += 1;
        }
        items.push(ExecutionResultItem {
            path: file.path.clone(),
            success,
            message,
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
