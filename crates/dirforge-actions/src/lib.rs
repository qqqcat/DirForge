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
