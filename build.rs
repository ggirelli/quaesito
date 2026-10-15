//! Combines the question library's topic files (`library/<category>/<category>-<topic>.json`,
//! each a JSON array of entries) into one array, which the server bundles.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    println!("cargo::rerun-if-changed=library");
    let mut files = Vec::new();
    collect(Path::new("library"), &mut files);
    files.sort();
    let mut entries = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        let inner = text
            .trim()
            .strip_prefix('[')
            .and_then(|t| t.strip_suffix(']'))
            .unwrap_or_else(|| panic!("{} is not a JSON array", file.display()))
            .trim();
        if !inner.is_empty() {
            entries.push(inner.to_owned());
        }
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    fs::write(
        out.join("library.json"),
        format!("[\n{}\n]\n", entries.join(",\n")),
    )
    .expect("the combined library can be written");
}

/// Every `.json` file under `dir`.
fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, files);
        } else if path.extension().is_some_and(|e| e == "json") {
            files.push(path);
        }
    }
}
