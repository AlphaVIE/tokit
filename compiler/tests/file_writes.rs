mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use tokit_compiler::{
    check,
    filesystem::{ReadError, WritePolicy},
    run_with_capabilities,
};

fn temporary_directory() -> PathBuf {
    let nonce = common::nonce();
    let directory =
        std::env::temp_dir().join(format!("tokit-write-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    directory
}

fn literal(path: &Path) -> String {
    format!(
        "\"{}\"",
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    )
}

fn write_source(path: &Path, text: &str) -> String {
    format!(
        "fn main()->Result<Unit,IoError>{{write_text({},{text:?})}}",
        literal(path)
    )
}

#[test]
fn write_policy_requires_its_own_grant_and_contains_targets() {
    let directory = temporary_directory();
    let allowed = directory.join("allowed");
    std::fs::create_dir(&allowed).unwrap();
    let outside = directory.join("outside.txt");
    std::fs::write(&outside, "outside").unwrap();
    let destination = allowed.join("new.txt");
    let denied = WritePolicy::from_root(None);
    assert_eq!(
        denied.write_text(&destination.to_string_lossy(), "x"),
        Err(ReadError::Denied)
    );

    let policy = WritePolicy::from_root(Some(&allowed));
    policy
        .write_text(&destination.to_string_lossy(), "first")
        .unwrap();
    policy
        .write_text(&destination.to_string_lossy(), "second")
        .unwrap();
    assert_eq!(std::fs::read_to_string(&destination).unwrap(), "second");
    assert_eq!(
        policy.write_text(&outside.to_string_lossy(), "bad"),
        Err(ReadError::Denied)
    );
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "outside");
    assert_eq!(
        policy.write_text(&allowed.join("missing/child.txt").to_string_lossy(), "bad"),
        Err(ReadError::Denied)
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn interpreter_and_native_require_write_grant_separately_from_read() {
    let directory = temporary_directory();
    let path = directory.join("output.txt");
    let source = write_source(&path, "hello\nworld");
    assert_eq!(
        run_with_capabilities(&source, Some(&directory), None, &[])
            .unwrap()
            .to_string(),
        "Err(IoError::Denied)"
    );
    assert!(!path.exists());
    assert_eq!(
        run_with_capabilities(&source, None, Some(&directory), &[])
            .unwrap()
            .to_string(),
        "Ok(())"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello\nworld");

    let source_path = directory.join("write.tok");
    std::fs::write(&source_path, &source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("run")
        .arg("--allow-write")
        .arg(&directory)
        .arg(&source_path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "Ok(())");

    let binary = directory.join(format!("native{}", std::env::consts::EXE_SUFFIX));
    let build = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    std::fs::remove_file(&path).unwrap();
    let denied = Command::new(&binary).output().unwrap();
    assert!(denied.status.success());
    assert_eq!(
        String::from_utf8(denied.stdout).unwrap().trim(),
        "Err(IoError::Denied)"
    );
    assert!(!path.exists());
    let granted = Command::new(&binary)
        .arg("--allow-write")
        .arg(&directory)
        .output()
        .unwrap();
    assert!(
        granted.status.success(),
        "{}",
        String::from_utf8_lossy(&granted.stderr)
    );
    assert_eq!(String::from_utf8(granted.stdout).unwrap().trim(), "Ok(())");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello\nworld");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn write_text_is_typed_reserved_and_forbidden_in_spawned_pure_functions() {
    for (source, code) in [
        (
            "fn main()->Result<Unit,IoError>{write_text(1,\"x\")}",
            "E102",
        ),
        ("fn main()->Result<Unit,IoError>{write_text(\"x\")}", "E105"),
        ("fn write_text()->i32{1} fn main()->i32{0}", "E106"),
        (
            "fn f()->Result<Unit,IoError>{write_text(\"x\",\"y\")} fn main()->Task<Result<Unit,IoError>>{spawn f()}",
            "E117",
        ),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

#[test]
fn file_copy_uses_separate_grants_and_preserves_program_arguments() {
    let directory = temporary_directory();
    let input_dir = directory.join("input");
    let output_dir = directory.join("output");
    std::fs::create_dir(&input_dir).unwrap();
    std::fs::create_dir(&output_dir).unwrap();
    let input = input_dir.join("source.txt");
    let output = output_dir.join("copy.txt");
    std::fs::write(&input, "Grüß, 世界\n").unwrap();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let source_path = workspace.join("examples/file_copy.tok");
    let source = std::fs::read_to_string(&source_path).unwrap();
    let args = vec![input.display().to_string(), output.display().to_string()];
    assert_eq!(
        run_with_capabilities(&source, Some(&input_dir), Some(&output_dir), &args)
            .unwrap()
            .to_string(),
        "Ok(())"
    );
    assert_eq!(std::fs::read_to_string(&output).unwrap(), "Grüß, 世界\n");
    std::fs::remove_file(&output).unwrap();

    let cli = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["run", "--allow-write"])
        .arg(&output_dir)
        .arg("--allow-read")
        .arg(&input_dir)
        .arg(&source_path)
        .arg("--")
        .args(&args)
        .output()
        .unwrap();
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    assert_eq!(String::from_utf8(cli.stdout).unwrap().trim(), "Ok(())");
    std::fs::remove_file(&output).unwrap();

    let binary = directory.join(format!("copy{}", std::env::consts::EXE_SUFFIX));
    let build = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(&binary)
        .arg("--allow-read")
        .arg(&input_dir)
        .arg("--allow-write")
        .arg(&output_dir)
        .arg("--")
        .args(&args)
        .output()
        .unwrap();
    assert!(
        native.status.success(),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    assert_eq!(String::from_utf8(native.stdout).unwrap().trim(), "Ok(())");
    assert_eq!(std::fs::read_to_string(&output).unwrap(), "Grüß, 世界\n");
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn write_policy_rejects_links_that_escape_grant() {
    use std::os::unix::fs::symlink;
    let directory = temporary_directory();
    let allowed = directory.join("allowed");
    std::fs::create_dir(&allowed).unwrap();
    let outside = directory.join("outside.txt");
    std::fs::write(&outside, "safe").unwrap();
    symlink(&outside, allowed.join("escape.txt")).unwrap();
    symlink(directory.join("missing.txt"), allowed.join("dangling.txt")).unwrap();
    let policy = WritePolicy::from_root(Some(&allowed));
    assert_eq!(
        policy.write_text(&allowed.join("escape.txt").to_string_lossy(), "bad"),
        Err(ReadError::Denied)
    );
    assert_eq!(
        policy.write_text(&allowed.join("dangling.txt").to_string_lossy(), "bad"),
        Err(ReadError::Denied)
    );
    let source_path = directory.join("escape.tok");
    std::fs::write(
        &source_path,
        write_source(&allowed.join("escape.txt"), "bad"),
    )
    .unwrap();
    let binary = directory.join("escape");
    let build = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(&binary)
        .arg("--allow-write")
        .arg(&allowed)
        .output()
        .unwrap();
    assert!(native.status.success());
    assert_eq!(
        String::from_utf8(native.stdout).unwrap().trim(),
        "Err(IoError::Denied)"
    );
    assert_eq!(std::fs::read_to_string(outside).unwrap(), "safe");
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(windows)]
#[test]
fn write_policy_rejects_symlink_escape_when_available() {
    let directory = temporary_directory();
    let allowed = directory.join("allowed");
    std::fs::create_dir(&allowed).unwrap();
    let outside = directory.join("outside.txt");
    std::fs::write(&outside, "safe").unwrap();
    let link = allowed.join("escape.txt");
    if std::os::windows::fs::symlink_file(&outside, &link).is_ok() {
        let policy = WritePolicy::from_root(Some(&allowed));
        assert_eq!(
            policy.write_text(&link.to_string_lossy(), "bad"),
            Err(ReadError::Denied)
        );
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "safe");
    }
    std::fs::remove_dir_all(directory).unwrap();
}
