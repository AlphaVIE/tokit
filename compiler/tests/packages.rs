use std::path::{Path, PathBuf};
use std::process::Command;

use tokit_compiler::{ai_index, interpreter, modules, native, packages};

fn temporary_directory() -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    for attempt in 0..100 {
        let path = std::env::temp_dir().join(format!(
            "tokit-package-{}-{nonce}-{attempt}",
            std::process::id()
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => return path,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("cannot create {}: {error}", path.display()),
        }
    }
    panic!("could not find a free package test directory")
}

fn clean_fixture(directory: &Path) {
    let target = directory.canonicalize().unwrap();
    let temp = std::env::temp_dir().canonicalize().unwrap();
    assert!(target.starts_with(temp));
    assert!(
        target
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("tokit-package-")
    );
    std::fs::remove_dir_all(target).unwrap();
}

fn fixture(directory: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let app = directory.join("app");
    let library = directory.join("library");
    std::fs::create_dir(&app).unwrap();
    std::fs::create_dir(&library).unwrap();
    let module = library.join("math.tok");
    std::fs::write(&module, "pub fn twice(n:i32)->i32{n*2}").unwrap();
    let digest = packages::hash_file(&module).unwrap();
    std::fs::write(
        app.join("tok.toml"),
        format!(
            "[dependencies]\nmath = {{ path = \"../library/math.tok\", sha256 = \"{digest}\" }}\n"
        ),
    )
    .unwrap();
    let entry = app.join("main.tok");
    std::fs::write(
        &entry,
        "import math=\"pkg:math\";fn main()->i32{math::twice(7)}",
    )
    .unwrap();
    (entry, module, app.join("tok.toml"))
}

#[test]
fn pinned_package_import_runs_without_absolute_names() {
    let directory = temporary_directory();
    let (entry, module, _) = fixture(&directory);
    let loaded = modules::load(&entry).unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(interpreter::run(&loaded.program).unwrap().to_string(), "14");
    assert_eq!(
        loaded
            .sources
            .display_path(tokit_compiler::ast::SourceId(1))
            .unwrap(),
        Path::new("pkg/math.tok")
    );
    let index = ai_index::index_loaded(&loaded);
    assert!(index.contains("\"sources\":[\"main.tok\",\"pkg/math.tok\"]"));
    assert!(index.contains("pkg::math::twice"));
    assert!(!index.contains(&directory.to_string_lossy().to_string()));

    let hash = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["pkg-hash", module.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(hash.status.success());
    assert_eq!(
        String::from_utf8(hash.stdout).unwrap().trim(),
        packages::hash_file(&module).unwrap()
    );
    if Command::new("rustc").arg("--version").output().is_ok() {
        let output = directory.join(format!("package-program{}", std::env::consts::EXE_SUFFIX));
        native::build_with_sources(&loaded.program, &loaded.sources, &output).unwrap();
        let result = Command::new(output).output().unwrap();
        assert!(result.status.success());
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), "14");
    } else {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    clean_fixture(&directory);
}

#[test]
fn package_manifest_and_content_are_enforced() {
    let directory = temporary_directory();
    let (entry, module, manifest) = fixture(&directory);
    let verified = packages::load(entry.parent().unwrap()).unwrap();
    std::fs::write(&module, "pub fn twice(n:i32)->i32{n*3}").unwrap();
    assert_eq!(
        verified.get("math").unwrap().source,
        "pub fn twice(n:i32)->i32{n*2}"
    );
    let error = modules::load(&entry).err().unwrap();
    assert_eq!(error.diagnostic.code, "E120");
    assert!(error.diagnostic.message.contains("sha256 mismatch"));

    std::fs::write(
        &manifest,
        "[dependencies]\nmath = { path = \"../library/math.tok\", sha256 = \"bad\" }\n",
    )
    .unwrap();
    assert_eq!(modules::load(&entry).err().unwrap().diagnostic.code, "E120");
    std::fs::write(&manifest, "[dependencies]\n").unwrap();
    assert!(
        modules::load(&entry)
            .err()
            .unwrap()
            .diagnostic
            .message
            .contains("unknown dependency")
    );
    std::fs::remove_file(&manifest).unwrap();
    assert_eq!(modules::load(&entry).err().unwrap().diagnostic.code, "E120");
    clean_fixture(&directory);
}

#[test]
fn package_files_cannot_escape_the_single_file_contract() {
    let directory = temporary_directory();
    let (entry, module, manifest) = fixture(&directory);
    std::fs::write(
        module.parent().unwrap().join("extra.tok"),
        "pub fn triple()->i32{3}",
    )
    .unwrap();
    std::fs::write(
        &module,
        "import extra=\"extra.tok\";pub fn twice(n:i32)->i32{n*2}",
    )
    .unwrap();
    let digest = packages::hash_file(&module).unwrap();
    std::fs::write(
        &manifest,
        format!(
            "[dependencies]\nmath = {{ path = \"../library/math.tok\", sha256 = \"{digest}\" }}\n"
        ),
    )
    .unwrap();
    let error = modules::load(&entry).err().unwrap();
    assert_eq!(error.diagnostic.code, "E120");
    assert!(error.diagnostic.message.contains("cannot import"));

    std::fs::write(
        &entry,
        "import extra=\"../library/extra.tok\";fn main()->i32{1}",
    )
    .unwrap();
    assert_eq!(modules::load(&entry).err().unwrap().diagnostic.code, "E118");
    clean_fixture(&directory);
}

#[test]
fn one_file_cannot_have_relative_and_package_identities() {
    let directory = temporary_directory();
    let (entry, module, manifest) = fixture(&directory);
    let local = entry.parent().unwrap().join("math.tok");
    std::fs::copy(&module, &local).unwrap();
    let digest = packages::hash_file(&local).unwrap();
    std::fs::write(
        &manifest,
        format!("[dependencies]\nmath = {{ path = \"math.tok\", sha256 = \"{digest}\" }}\n"),
    )
    .unwrap();
    for imports in [
        "import direct=\"math.tok\";import packaged=\"pkg:math\";",
        "import packaged=\"pkg:math\";import direct=\"math.tok\";",
    ] {
        std::fs::write(&entry, format!("{imports}fn main()->i32{{1}}")).unwrap();
        assert_eq!(modules::load(&entry).err().unwrap().diagnostic.code, "E120");
    }
    clean_fixture(&directory);
}

#[test]
fn packaged_json_example_runs_in_both_backends() {
    let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples/package_json/main.tok");
    let loaded = modules::load(&entry).unwrap_or_else(|error| panic!("{}", error.display()));
    let expected = "Some(\"{\\\"ok\\\":true,\\\"items\\\":[1,null]}\")";
    assert_eq!(
        interpreter::run(&loaded.program).unwrap().to_string(),
        expected
    );
    if Command::new("rustc").arg("--version").output().is_ok() {
        let directory = temporary_directory();
        let output = directory.join(format!("json{}", std::env::consts::EXE_SUFFIX));
        native::build_with_sources(&loaded.program, &loaded.sources, &output).unwrap();
        let result = Command::new(output).output().unwrap();
        assert!(result.status.success());
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), expected);
        clean_fixture(&directory);
    } else {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
}

#[test]
fn package_diagnostics_use_logical_source_path() {
    let directory = temporary_directory();
    let (entry, module, manifest) = fixture(&directory);
    std::fs::write(&module, "pub fn twice(n:i32)->i32{true}").unwrap();
    let digest = packages::hash_file(&module).unwrap();
    std::fs::write(
        &manifest,
        format!(
            "[dependencies]\nmath = {{ path = \"../library/math.tok\", sha256 = \"{digest}\" }}\n"
        ),
    )
    .unwrap();
    let error = modules::load(&entry).err().unwrap();
    assert_eq!(error.diagnostic.code, "E102");
    assert!(error.display().contains("pkg/math.tok:E102"));
    assert!(
        !error
            .display()
            .contains(&directory.to_string_lossy().to_string())
    );
    clean_fixture(&directory);
}
