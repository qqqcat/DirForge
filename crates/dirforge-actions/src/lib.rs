#[derive(Debug, Clone)]
pub struct DeletionPlan {
    pub files: Vec<String>,
    pub reclaimable_bytes: u64,
}

pub fn build_deletion_plan(files: Vec<(String, u64)>) -> DeletionPlan {
    let reclaimable_bytes = files.iter().map(|(_, s)| *s).sum();
    DeletionPlan {
        files: files.into_iter().map(|(p, _)| p).collect(),
        reclaimable_bytes,
    }
}
