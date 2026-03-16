use dirforge_core::{ErrorKind, NodeKind, NodeStore, ScanErrorRecord};
use serde::Serialize;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct SummaryRow {
    pub path: String,
    pub kind: &'static str,
    pub size_self: u64,
    pub size_subtree: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SummaryReport {
    pub nodes: usize,
    pub rows: Vec<SummaryRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateRow {
    pub group: usize,
    pub size: u64,
    pub path: String,
    pub keeper: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateReport {
    pub groups: usize,
    pub rows: Vec<DuplicateRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorRow {
    pub path: String,
    pub reason: String,
    pub kind: ErrorKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorReport {
    pub count: usize,
    pub rows: Vec<ErrorRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticsBundleManifest {
    pub diagnostics_payload_file: String,
    pub summary_report_file: String,
    pub duplicate_report_file: String,
    pub error_report_file: String,
    pub format: &'static str,
    pub structure_version: u32,
}

pub const DIAGNOSTICS_BUNDLE_STRUCTURE_VERSION: u32 = 2;

pub fn default_manifest() -> DiagnosticsBundleManifest {
    DiagnosticsBundleManifest {
        diagnostics_payload_file: "diagnostics.json".to_string(),
        summary_report_file: "summary.json".to_string(),
        duplicate_report_file: "duplicates.csv".to_string(),
        error_report_file: "errors.csv".to_string(),
        format: "json",
        structure_version: DIAGNOSTICS_BUNDLE_STRUCTURE_VERSION,
    }
}
pub fn build_summary_report(store: &NodeStore) -> SummaryReport {
    let rows = store
        .nodes
        .iter()
        .map(|n| SummaryRow {
            path: n.path.clone(),
            kind: if matches!(n.kind, NodeKind::Dir) {
                "dir"
            } else {
                "file"
            },
            size_self: n.size_self,
            size_subtree: n.size_subtree,
        })
        .collect();

    SummaryReport {
        nodes: store.nodes.len(),
        rows,
    }
}

pub fn build_duplicate_report(groups: &[dirforge_dup::DuplicateGroup]) -> DuplicateReport {
    let mut rows = Vec::new();
    for (group_idx, group) in groups.iter().enumerate() {
        for member in &group.members {
            rows.push(DuplicateRow {
                group: group_idx,
                size: group.size,
                path: member.path.clone(),
                keeper: member.keeper,
            });
        }
    }
    DuplicateReport {
        groups: groups.len(),
        rows,
    }
}

pub fn build_error_report(errors: &[ScanErrorRecord]) -> ErrorReport {
    ErrorReport {
        count: errors.len(),
        rows: errors
            .iter()
            .map(|e| ErrorRow {
                path: e.path.clone(),
                reason: e.reason.clone(),
                kind: e.kind,
            })
            .collect(),
    }
}

pub fn export_summary_txt(store: &NodeStore, output: impl AsRef<Path>) -> io::Result<()> {
    let report = build_summary_report(store);
    let mut out = String::new();
    out.push_str("DirForge Summary Report\n");
    out.push_str(&format!("nodes={}\n", report.nodes));
    for row in report.rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            row.path, row.kind, row.size_self, row.size_subtree
        ));
    }
    fs::write(output, out)
}

pub fn export_summary_json(store: &NodeStore, output: impl AsRef<Path>) -> io::Result<()> {
    let report = build_summary_report(store);
    let payload = serde_json::to_vec_pretty(&report)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(output, payload)
}

pub fn export_duplicates_csv(
    groups: &[dirforge_dup::DuplicateGroup],
    output: impl AsRef<Path>,
) -> io::Result<()> {
    let report = build_duplicate_report(groups);
    let mut out = String::from("group,size,path,keeper\n");
    for row in report.rows {
        out.push_str(&format!(
            "{},{},\"{}\",{}\n",
            row.group,
            row.size,
            row.path.replace('"', "\"\""),
            row.keeper
        ));
    }
    fs::write(output, out)
}

pub fn export_errors_csv(errors: &[ScanErrorRecord], output: impl AsRef<Path>) -> io::Result<()> {
    let report = build_error_report(errors);
    let mut out = String::from("path,reason,kind\n");
    for row in report.rows {
        out.push_str(&format!(
            "\"{}\",\"{}\",{:?}\n",
            row.path.replace('"', "\"\""),
            row.reason.replace('"', "\"\""),
            row.kind
        ));
    }
    fs::write(output, out)
}

pub fn export_diagnostics_bundle(
    payload: &str,
    output: impl AsRef<Path>,
    manifest: &DiagnosticsBundleManifest,
) -> io::Result<()> {
    let output_path = output.as_ref();
    fs::write(output_path, payload)?;
    let manifest_path = output_path.with_extension("manifest.json");
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(manifest_path, bytes)
}

pub fn export_text_report(store: &NodeStore, output: impl AsRef<Path>) -> io::Result<()> {
    export_summary_txt(store, output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dirforge_core::{NodeKind, NodeStore, RiskLevel};
    use dirforge_dup::{DuplicateGroup, DuplicateMember};

    #[test]
    fn export_report_smoke() {
        let mut store = NodeStore::default();
        let root = store.add_node(None, "root".into(), "/tmp/root".into(), NodeKind::Dir, 0);
        store.add_node(
            Some(root),
            "a".into(),
            "/tmp/root/a".into(),
            NodeKind::File,
            42,
        );
        store.rollup();

        let out = std::env::temp_dir().join("dirforge_report_test.txt");
        export_summary_txt(&store, &out).expect("export report");
        let content = std::fs::read_to_string(&out).expect("read report");
        assert!(content.contains("DirForge Summary Report"));
        let _ = std::fs::remove_file(out);
    }

    #[test]
    fn export_errors_and_duplicates_csv_smoke() {
        let err_out = std::env::temp_dir().join("dirforge_errors_test.csv");
        let dup_out = std::env::temp_dir().join("dirforge_dups_test.csv");

        let errors = vec![ScanErrorRecord {
            path: "/tmp/nope".into(),
            reason: "denied".into(),
            kind: ErrorKind::User,
        }];
        export_errors_csv(&errors, &err_out).expect("errors csv");

        let groups = vec![DuplicateGroup {
            size: 10,
            members: vec![
                DuplicateMember {
                    path: "/tmp/a".into(),
                    size: 10,
                    keeper: true,
                },
                DuplicateMember {
                    path: "/tmp/b".into(),
                    size: 10,
                    keeper: false,
                },
            ],
            reclaimable_bytes: 10,
            risk: RiskLevel::Low,
        }];
        export_duplicates_csv(&groups, &dup_out).expect("dups csv");

        let err_content = std::fs::read_to_string(&err_out).expect("read errors");
        let dup_content = std::fs::read_to_string(&dup_out).expect("read dups");
        assert!(err_content.contains("path,reason,kind"));
        assert!(dup_content.contains("group,size,path,keeper"));

        let _ = std::fs::remove_file(err_out);
        let _ = std::fs::remove_file(dup_out);
    }

    #[test]
    fn export_diagnostics_manifest_smoke() {
        let out = std::env::temp_dir().join("dirforge_diag_test.json");
        let mut manifest = default_manifest();
        manifest.diagnostics_payload_file = "dirforge_diag_test.json".into();
        export_diagnostics_bundle("{\"ok\":true}", &out, &manifest).expect("export diagnostics");
        let content = std::fs::read_to_string(&out).expect("read diagnostics");
        assert!(content.contains("ok"));
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(out.with_extension("manifest.json"));
    }
}
