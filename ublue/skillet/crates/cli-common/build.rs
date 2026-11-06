use vergen_gix::{Emitter, Gix};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Dirty means tracked changes. Untracked files do not identify deployed source.
    let git = Gix::builder().sha(false).dirty(false).build();
    Emitter::default().add_instructions(&git)?.emit()?;
    if let Ok(repo) = gix::discover(env!("CARGO_MANIFEST_DIR")) {
        if let Some(worktree) = repo.workdir() {
            let index = repo.index_or_load_from_head_or_empty()?;
            for entry in index.entries() {
                let path = worktree.join(gix::path::from_bstr(entry.path(&index)));
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
        for directory in [repo.path(), repo.common_dir()] {
            for name in ["index", "packed-refs", "refs"] {
                let path = directory.join(name);
                if path.exists() {
                    println!("cargo:rerun-if-changed={}", path.display());
                }
            }
        }
    }
    Ok(())
}
