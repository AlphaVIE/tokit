mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use tokit_compiler::{ai_index, interpreter, modules, native, packages};

fn temporary_directory() -> PathBuf {
    let nonce = common::nonce();
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
    packages::write_lock(&app).unwrap();
    (entry, module, app.join("tok.toml"))
}

fn tree_fixture(directory: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let app = directory.join("app");
    let library = directory.join("library");
    std::fs::create_dir(&app).unwrap();
    std::fs::create_dir_all(library.join("lib")).unwrap();
    std::fs::write(
        library.join("api.tok"),
        "import helper=\"lib/helper.tok\";pub fn answer()->i32{helper::double(21)}",
    )
    .unwrap();
    std::fs::write(
        library.join("lib/helper.tok"),
        "pub fn double(n:i32)->i32{n*2}",
    )
    .unwrap();
    std::fs::write(library.join("unused.tok"), "fn unused()->i32{0}").unwrap();
    let digest = packages::hash_path(&library).unwrap();
    let manifest = app.join("tok.toml");
    std::fs::write(
        &manifest,
        format!(
            "[dependencies]\ncalc = {{ path = \"../library\", entry = \"api.tok\", sha256 = \"{digest}\" }}\n"
        ),
    )
    .unwrap();
    let entry = app.join("main.tok");
    std::fs::write(
        &entry,
        "import calc=\"pkg:calc\";fn main()->i32{calc::answer()}",
    )
    .unwrap();
    packages::write_lock(&app).unwrap();
    (entry, library, manifest)
}

fn transitive_fixture(directory: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let app = directory.join("app");
    let core = directory.join("core");
    let util = directory.join("util");
    std::fs::create_dir(&app).unwrap();
    std::fs::create_dir(&core).unwrap();
    std::fs::create_dir(&util).unwrap();
    std::fs::write(util.join("api.tok"), "pub fn triple(n:i32)->i32{n*3}").unwrap();
    let util_hash = packages::hash_path(&util).unwrap();
    std::fs::write(
        core.join("tok.toml"),
        format!("[dependencies]\nutil = {{ path = \"../util\", entry = \"api.tok\", sha256 = \"{util_hash}\" }}\n"),
    ).unwrap();
    std::fs::write(
        core.join("api.tok"),
        "import util=\"pkg:util\";pub fn answer()->i32{util::triple(14)}",
    )
    .unwrap();
    let core_hash = packages::hash_path(&core).unwrap();
    std::fs::write(
        app.join("tok.toml"),
        format!("[dependencies]\ncore = {{ path = \"../core\", entry = \"api.tok\", sha256 = \"{core_hash}\" }}\n"),
    ).unwrap();
    let entry = app.join("main.tok");
    std::fs::write(
        &entry,
        "import core=\"pkg:core\";fn main()->i32{core::answer()}",
    )
    .unwrap();
    packages::write_lock(&app).unwrap();
    (entry, core, util)
}

#[test]
fn transitive_package_graph_is_pinned_portable_and_runs_in_both_backends() {
    let first = temporary_directory();
    let second = temporary_directory();
    let (entry, core, util) = transitive_fixture(&first);
    let (other_entry, _, _) = transitive_fixture(&second);
    let lock = std::fs::read_to_string(entry.parent().unwrap().join("tok.lock")).unwrap();
    assert_eq!(
        lock,
        std::fs::read_to_string(other_entry.parent().unwrap().join("tok.lock")).unwrap()
    );
    assert!(lock.contains("format = 2"));
    assert!(lock.contains("id = \"core/util\""));
    assert!(lock.contains("target = \"core/util\""));
    assert!(lock.contains("pkg/@/core/util/api.tok"));
    assert!(!lock.contains(&first.to_string_lossy().to_string()));
    let loaded = modules::load(&entry).unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(interpreter::run(&loaded.program).unwrap().to_string(), "42");
    assert_eq!(loaded.sources.len(), 3);
    if Command::new("rustc").arg("--version").output().is_ok() {
        let binary = first.join(format!("transitive{}", std::env::consts::EXE_SUFFIX));
        native::build_with_sources(&loaded.program, &loaded.sources, &binary).unwrap();
        let output = Command::new(&binary).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "42");
    } else {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    let core_manifest = core.join("tok.toml");
    let original_manifest = std::fs::read(&core_manifest).unwrap();
    std::fs::write(&core_manifest, "[dependencies]\n").unwrap();
    let error = modules::load(&entry).err().unwrap();
    assert_eq!(error.diagnostic.code, "E120");
    assert!(error.diagnostic.message.contains("sha256 mismatch"));
    std::fs::write(&core_manifest, original_manifest).unwrap();
    std::fs::write(util.join("api.tok"), "pub fn triple(n:i32)->i32{n*4}").unwrap();
    let error = modules::load(&entry).err().unwrap();
    assert_eq!(error.diagnostic.code, "E120");
    assert!(error.diagnostic.message.contains("sha256 mismatch"));
    clean_fixture(&first);
    clean_fixture(&second);
}

#[test]
fn shared_transitive_package_has_one_identity_and_conflicting_pins_fail() {
    let directory = temporary_directory();
    let app = directory.join("app");
    let shared = directory.join("shared");
    let left = directory.join("left");
    let right = directory.join("right");
    for path in [&app, &shared, &left, &right] {
        std::fs::create_dir(path).unwrap();
    }
    std::fs::write(shared.join("api.tok"), "pub fn value()->i32{21}").unwrap();
    let shared_hash = packages::hash_path(&shared).unwrap();
    for (path, function) in [(&left, "a"), (&right, "b")] {
        std::fs::write(
            path.join("tok.toml"),
            format!("[dependencies]\nshared = {{ path = \"../shared\", entry = \"api.tok\", sha256 = \"{shared_hash}\" }}\n"),
        )
        .unwrap();
        std::fs::write(
            path.join("api.tok"),
            format!("import shared=\"pkg:shared\";pub fn {function}()->i32{{shared::value()}}"),
        )
        .unwrap();
    }
    let left_hash = packages::hash_path(&left).unwrap();
    let right_hash = packages::hash_path(&right).unwrap();
    std::fs::write(
        app.join("tok.toml"),
        format!("[dependencies]\nleft = {{ path = \"../left\", entry = \"api.tok\", sha256 = \"{left_hash}\" }}\nright = {{ path = \"../right\", entry = \"api.tok\", sha256 = \"{right_hash}\" }}\n"),
    )
    .unwrap();
    let entry = app.join("main.tok");
    std::fs::write(
        &entry,
        "import left=\"pkg:left\";import right=\"pkg:right\";fn main()->i32{left::a()+right::b()}",
    )
    .unwrap();
    packages::write_lock(&app).unwrap();
    let lock = std::fs::read_to_string(app.join("tok.lock")).unwrap();
    assert_eq!(lock.matches("id = \"left/shared\"").count(), 1);
    assert_eq!(lock.matches("target = \"left/shared\"").count(), 2);
    let loaded = modules::load(&entry).unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(loaded.sources.len(), 4);
    assert_eq!(interpreter::run(&loaded.program).unwrap().to_string(), "42");

    std::fs::write(
        right.join("tok.toml"),
        format!("[dependencies]\nshared = {{ path = \"../shared\", entry = \"api.tok\", sha256 = \"{}\" }}\n", "0".repeat(64)),
    )
    .unwrap();
    let right_hash = packages::hash_path(&right).unwrap();
    std::fs::write(
        app.join("tok.toml"),
        format!("[dependencies]\nleft = {{ path = \"../left\", entry = \"api.tok\", sha256 = \"{left_hash}\" }}\nright = {{ path = \"../right\", entry = \"api.tok\", sha256 = \"{right_hash}\" }}\n"),
    )
    .unwrap();
    assert!(
        packages::write_lock(&app)
            .unwrap_err()
            .contains("conflicting checksums")
    );
    clean_fixture(&directory);
}

#[test]
fn transitive_dependency_cycle_is_rejected() {
    let directory = temporary_directory();
    let (entry, core, _) = transitive_fixture(&directory);
    std::fs::write(
        core.join("tok.toml"),
        format!(
            "[dependencies]\nself = {{ path = \".\", entry = \"api.tok\", sha256 = \"{}\" }}\n",
            "0".repeat(64)
        ),
    )
    .unwrap();
    let core_hash = packages::hash_path(&core).unwrap();
    std::fs::write(
        entry.parent().unwrap().join("tok.toml"),
        format!("[dependencies]\ncore = {{ path = \"../core\", entry = \"api.tok\", sha256 = \"{core_hash}\" }}\n"),
    )
    .unwrap();
    assert!(
        packages::write_lock(entry.parent().unwrap())
            .unwrap_err()
            .contains("cycle")
    );
    clean_fixture(&directory);
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
    packages::write_lock(entry.parent().unwrap()).unwrap();
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
    packages::write_lock(entry.parent().unwrap()).unwrap();
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
    packages::write_lock(entry.parent().unwrap()).unwrap();
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
    packages::write_lock(entry.parent().unwrap()).unwrap();
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

#[test]
fn pinned_package_tree_loads_relative_modules_in_both_backends() {
    let directory = temporary_directory();
    let (entry, library, _) = tree_fixture(&directory);
    let lock = std::fs::read_to_string(entry.parent().unwrap().join("tok.lock")).unwrap();
    assert!(lock.contains("pkg/calc/unused.tok"));
    let loaded = modules::load(&entry).unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(interpreter::run(&loaded.program).unwrap().to_string(), "42");
    let index = ai_index::index_loaded(&loaded);
    assert!(index.contains("pkg/calc/api.tok"));
    assert!(index.contains("pkg/calc/lib/helper.tok"));
    assert!(index.contains("pkg::calc::lib::helper::double"));
    assert!(!index.contains(&directory.to_string_lossy().to_string()));
    assert_eq!(loaded.sources.len(), 3);

    let hash = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["pkg-hash", library.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(hash.status.success());
    assert_eq!(
        String::from_utf8(hash.stdout).unwrap().trim(),
        packages::hash_path(&library).unwrap()
    );
    if Command::new("rustc").arg("--version").output().is_ok() {
        let output = directory.join(format!("tree-program{}", std::env::consts::EXE_SUFFIX));
        native::build_with_sources(&loaded.program, &loaded.sources, &output).unwrap();
        let result = Command::new(output).output().unwrap();
        assert!(result.status.success());
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), "42");
    } else {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    clean_fixture(&directory);
}

#[test]
fn tree_hash_covers_unused_sources_and_rejects_escape() {
    let directory = temporary_directory();
    let (entry, library, manifest) = tree_fixture(&directory);
    let original = packages::hash_path(&library).unwrap();
    std::fs::write(library.join("unused.tok"), "fn unused()->i32{1}").unwrap();
    assert_ne!(packages::hash_path(&library).unwrap(), original);
    let error = modules::load(&entry).err().unwrap();
    assert_eq!(error.diagnostic.code, "E120");
    assert!(error.diagnostic.message.contains("sha256 mismatch"));

    std::fs::write(
        library.join("api.tok"),
        "import outside=\"../outside.tok\";pub fn answer()->i32{0}",
    )
    .unwrap();
    std::fs::write(directory.join("outside.tok"), "pub fn value()->i32{1}").unwrap();
    let digest = packages::hash_path(&library).unwrap();
    std::fs::write(
        &manifest,
        format!(
            "[dependencies]\ncalc = {{ path = \"../library\", entry = \"api.tok\", sha256 = \"{digest}\" }}\n"
        ),
    )
    .unwrap();
    packages::write_lock(entry.parent().unwrap()).unwrap();
    let error = modules::load(&entry).err().unwrap();
    assert_eq!(error.diagnostic.code, "E120");
    assert!(
        error
            .diagnostic
            .message
            .contains("escapes the package directory")
    );

    std::fs::write(
        library.join("api.tok"),
        "import other=\"pkg:other\";pub fn answer()->i32{0}",
    )
    .unwrap();
    let digest = packages::hash_path(&library).unwrap();
    std::fs::write(
        &manifest,
        format!(
            "[dependencies]\ncalc = {{ path = \"../library\", entry = \"api.tok\", sha256 = \"{digest}\" }}\n"
        ),
    )
    .unwrap();
    packages::write_lock(entry.parent().unwrap()).unwrap();
    let error = modules::load(&entry).err().unwrap();
    assert_eq!(error.diagnostic.code, "E120");
    assert!(error.diagnostic.message.contains("unknown dependency"));
    clean_fixture(&directory);
}

#[test]
fn tree_hash_is_independent_of_file_creation_order() {
    let directory = temporary_directory();
    let first = directory.join("first");
    let second = directory.join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    for (root, names) in [(&first, ["a.tok", "b.tok"]), (&second, ["b.tok", "a.tok"])] {
        for name in names {
            std::fs::write(
                root.join(name),
                format!("fn value()->i32{{{}}}", name.len()),
            )
            .unwrap();
        }
    }
    assert_eq!(
        packages::hash_path(&first).unwrap(),
        packages::hash_path(&second).unwrap()
    );
    clean_fixture(&directory);
}

#[test]
fn repository_package_tree_example_has_current_pin() {
    let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples/package_tree/app/main.tok");
    let loaded = modules::load(&entry).unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(interpreter::run(&loaded.program).unwrap().to_string(), "42");
}

#[test]
fn package_tree_rejects_symlink_entries_when_supported() {
    let directory = temporary_directory();
    let (_, library, _) = tree_fixture(&directory);
    let target = library.join("api.tok");
    let link = library.join("linked.tok");
    #[cfg(unix)]
    let created = std::os::unix::fs::symlink(&target, &link);
    #[cfg(windows)]
    let created = std::os::windows::fs::symlink_file(&target, &link);
    if created.is_ok() {
        let error = packages::hash_path(&library).unwrap_err();
        assert!(error.contains("symlinks"));
    }
    clean_fixture(&directory);
}

#[test]
fn lockfile_is_required_and_cli_regenerates_it() {
    let directory = temporary_directory();
    let (entry, module, manifest) = fixture(&directory);
    let lock = entry.parent().unwrap().join("tok.lock");
    let original = std::fs::read_to_string(&lock).unwrap();
    assert!(original.contains(&format!(
        "compiler = \"tokit-compiler/{}\"",
        env!("CARGO_PKG_VERSION")
    )));
    assert!(original.contains("target = \"portable-source\""));
    assert!(original.contains("sources = [\"pkg/math.tok\"]"));
    assert!(!original.contains(&directory.to_string_lossy().to_string()));

    std::fs::remove_file(&lock).unwrap();
    assert_eq!(modules::load(&entry).err().unwrap().diagnostic.code, "E121");
    let checked = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["check", "--json", entry.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!checked.status.success());
    assert!(
        String::from_utf8(checked.stdout)
            .unwrap()
            .contains("\"code\":\"E121\"")
    );
    let command = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["lock", entry.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        command.status.success(),
        "{}",
        String::from_utf8_lossy(&command.stderr)
    );
    assert_eq!(std::fs::read_to_string(&lock).unwrap(), original);
    modules::load(&entry).unwrap_or_else(|error| panic!("{}", error.display()));

    std::fs::write(&lock, format!("{original}\n")).unwrap();
    assert_eq!(modules::load(&entry).err().unwrap().diagnostic.code, "E121");
    packages::write_lock(entry.parent().unwrap()).unwrap();
    assert_eq!(std::fs::read_to_string(&lock).unwrap(), original);

    std::fs::write(
        &manifest,
        format!("{}\n", std::fs::read_to_string(&manifest).unwrap()),
    )
    .unwrap();
    assert_eq!(modules::load(&entry).err().unwrap().diagnostic.code, "E121");
    packages::write_lock(entry.parent().unwrap()).unwrap();
    modules::load(&entry).unwrap_or_else(|error| panic!("{}", error.display()));

    std::fs::write(&module, "pub fn twice(n:i32)->i32{n*3}").unwrap();
    assert!(
        packages::write_lock(entry.parent().unwrap())
            .unwrap_err()
            .contains("sha256 mismatch")
    );
    clean_fixture(&directory);
}

#[test]
fn lockfile_is_independent_of_checkout_path() {
    let first = temporary_directory();
    let second = temporary_directory();
    let (first_entry, _, _) = fixture(&first);
    let (second_entry, _, _) = fixture(&second);
    let first_lock = std::fs::read(first_entry.parent().unwrap().join("tok.lock")).unwrap();
    let second_lock = std::fs::read(second_entry.parent().unwrap().join("tok.lock")).unwrap();
    assert_eq!(first_lock, second_lock);
    clean_fixture(&first);
    clean_fixture(&second);
}

#[test]
fn lock_command_refuses_symlink_destination_when_supported() {
    let directory = temporary_directory();
    let (entry, _, _) = fixture(&directory);
    let lock = entry.parent().unwrap().join("tok.lock");
    let outside = directory.join("outside.txt");
    std::fs::write(&outside, "keep").unwrap();
    std::fs::remove_file(&lock).unwrap();
    #[cfg(unix)]
    let created = std::os::unix::fs::symlink(&outside, &lock);
    #[cfg(windows)]
    let created = std::os::windows::fs::symlink_file(&outside, &lock);
    if created.is_ok() {
        assert!(
            packages::write_lock(entry.parent().unwrap())
                .unwrap_err()
                .contains("symlink")
        );
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep");
    }
    clean_fixture(&directory);
}

#[test]
fn cli_add_and_rm_keep_manifest_comments_and_lock_in_sync() {
    let directory = temporary_directory();
    let (entry, _, manifest) = fixture(&directory);
    let original = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        format!("# retained heading\n{original}# retained tail\n"),
    )
    .unwrap();
    packages::write_lock(entry.parent().unwrap()).unwrap();
    let extra = directory.join("library/extra.tok");
    std::fs::write(&extra, "pub fn value()->i32{5}").unwrap();
    let run = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_tok"))
            .args(arguments)
            .output()
            .unwrap()
    };
    let entry_path = entry.to_str().unwrap();
    let added = run(&["add", entry_path, "extra", "../library/extra.tok"]);
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    let updated = std::fs::read_to_string(&manifest).unwrap();
    assert!(updated.contains("# retained heading"));
    assert!(updated.contains("# retained tail"));
    assert!(updated.contains("extra = {"));
    let lock = std::fs::read_to_string(entry.parent().unwrap().join("tok.lock")).unwrap();
    assert!(lock.contains("name = \"extra\""));
    assert!(lock.contains("name = \"math\""));
    modules::load(&entry).unwrap_or_else(|error| panic!("{}", error.display()));

    let duplicate = run(&["add", entry_path, "extra", "../library/extra.tok"]);
    assert!(!duplicate.status.success());
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), updated);
    assert_eq!(
        std::fs::read_to_string(entry.parent().unwrap().join("tok.lock")).unwrap(),
        lock
    );

    let removed = run(&["rm", entry_path, "extra"]);
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    let result = std::fs::read_to_string(&manifest).unwrap();
    assert!(result.contains("# retained heading"));
    assert!(result.contains("# retained tail"));
    assert!(!result.contains("extra ="));
    let lock = std::fs::read_to_string(entry.parent().unwrap().join("tok.lock")).unwrap();
    assert!(!lock.contains("name = \"extra\""));
    modules::load(&entry).unwrap_or_else(|error| panic!("{}", error.display()));
    clean_fixture(&directory);
}

#[test]
fn cli_add_directory_validates_entry_and_preserves_files_on_failure() {
    let directory = temporary_directory();
    let (entry, _, manifest) = fixture(&directory);
    let package = directory.join("library/pack");
    std::fs::create_dir(&package).unwrap();
    std::fs::write(package.join("api.tok"), "pub fn value()->i32{8}").unwrap();
    let entry_path = entry.to_str().unwrap();
    let manifest_before = std::fs::read_to_string(&manifest).unwrap();
    let lock_path = entry.parent().unwrap().join("tok.lock");
    let lock_before = std::fs::read_to_string(&lock_path).unwrap();
    let run = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_tok"))
            .args(arguments)
            .output()
            .unwrap()
    };
    for arguments in [
        vec!["add", entry_path, "pack", "../library/pack"],
        vec![
            "add",
            entry_path,
            "pack",
            "../library/pack",
            "--entry",
            "missing.tok",
        ],
        vec![
            "add",
            entry_path,
            "pack",
            "../library/math.tok",
            "--entry",
            "api.tok",
        ],
    ] {
        assert!(!run(&arguments).status.success(), "{arguments:?}");
        assert_eq!(std::fs::read_to_string(&manifest).unwrap(), manifest_before);
        assert_eq!(std::fs::read_to_string(&lock_path).unwrap(), lock_before);
    }
    let added = run(&[
        "add",
        entry_path,
        "pack",
        "../library/pack",
        "--entry",
        "api.tok",
    ]);
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    std::fs::write(
        &entry,
        "import pack=\"pkg:pack\";fn main()->i32{pack::value()}",
    )
    .unwrap();
    let loaded = modules::load(&entry).unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(interpreter::run(&loaded.program).unwrap().to_string(), "8");
    clean_fixture(&directory);
}

#[test]
fn cli_add_creates_manifest_and_refuses_symlink_lock_without_writing() {
    let directory = temporary_directory();
    let app = directory.join("app");
    let library = directory.join("library");
    std::fs::create_dir(&app).unwrap();
    std::fs::create_dir(&library).unwrap();
    let entry = app.join("main.tok");
    std::fs::write(
        &entry,
        "import item=\"pkg:item\";fn main()->i32{item::value()}",
    )
    .unwrap();
    std::fs::write(library.join("item.tok"), "pub fn value()->i32{9}").unwrap();
    let command = || {
        Command::new(env!("CARGO_BIN_EXE_tok"))
            .args([
                "add",
                entry.to_str().unwrap(),
                "item",
                "../library/item.tok",
            ])
            .output()
            .unwrap()
    };
    let lock = app.join("tok.lock");
    let outside = directory.join("outside.txt");
    std::fs::write(&outside, "keep").unwrap();
    #[cfg(unix)]
    let created = std::os::unix::fs::symlink(&outside, &lock);
    #[cfg(windows)]
    let created = std::os::windows::fs::symlink_file(&outside, &lock);
    if created.is_ok() {
        assert!(!command().status.success());
        assert!(!app.join("tok.toml").exists());
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep");
        std::fs::remove_file(&lock).unwrap();
    }
    let result = command();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(app.join("tok.toml").exists());
    assert!(lock.exists());
    let loaded = modules::load(&entry).unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(interpreter::run(&loaded.program).unwrap().to_string(), "9");
    clean_fixture(&directory);
}
