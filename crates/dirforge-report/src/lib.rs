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
