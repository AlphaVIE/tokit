//! Embed the official package registry (`../packages`) into the `tok` binary.

use std::fs;
use std::path::{Path, PathBuf};

fn walk(directory: &Path, root: &Path, files: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, root, files);
        } else if path.extension().is_some_and(|ext| ext == "tok")
            || path.file_name().is_some_and(|name| name == "tok.toml")
        {
            let relative = path
                .strip_prefix(root)
                .expect("walked path is inside the registry")
                .components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            files.push((relative, path));
        }
    }
}

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let root = Path::new(&manifest).join("../packages");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    files.sort();
    let mut code = String::from(
        "/// Registry files as (`package/relative/path`, bytes).\npub(crate) static FILES: &[(&str, &[u8])] = &[\n",
    );
    for (relative, path) in files {
        code.push_str(&format!(
            "    ({relative:?}, include_bytes!({:?})),\n",
            path.to_string_lossy()
        ));
    }
    code.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    fs::write(out.join("registry_files.rs"), code).expect("write registry index");
}
