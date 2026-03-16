use dirforge_core::NodeStore;
use std::fs;
use std::io;
use std::path::Path;

pub fn export_text_report(store: &NodeStore, output: impl AsRef<Path>) -> io::Result<()> {
    let mut out = String::new();
    out.push_str("DirForge Report\n");
    out.push_str(&format!("nodes={}\n", store.nodes.len()));
    for n in &store.nodes {
        out.push_str(&format!(
            "{}\t{}\t{}\n",
            n.path, n.size_self, n.size_subtree
        ));
    }
    fs::write(output, out)
}

pub fn export_diagnostics_bundle(payload: &str, output: impl AsRef<Path>) -> io::Result<()> {
    fs::write(output, payload)
}


#[cfg(test)]
mod tests {
    use super::*;
    use dirforge_core::{NodeKind, NodeStore};

    #[test]
    fn export_report_smoke() {
        let mut store = NodeStore::default();
        let root = store.add_node(None, "root".into(), "/tmp/root".into(), NodeKind::Dir, 0);
        store.add_node(Some(root), "a".into(), "/tmp/root/a".into(), NodeKind::File, 42);
        store.rollup();

        let out = std::env::temp_dir().join("dirforge_report_test.txt");
        export_text_report(&store, &out).expect("export report");
        let content = std::fs::read_to_string(&out).expect("read report");
        assert!(content.contains("DirForge Report"));
        let _ = std::fs::remove_file(out);
    }

    #[test]
    fn export_diagnostics_smoke() {
        let out = std::env::temp_dir().join("dirforge_diag_test.json");
        export_diagnostics_bundle("{\"ok\":true}", &out).expect("export diagnostics");
        let content = std::fs::read_to_string(&out).expect("read diagnostics");
        assert!(content.contains("ok"));
        let _ = std::fs::remove_file(out);
    }
}
