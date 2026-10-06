mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use tokit_compiler::filesystem::{IoError, ReadPolicy, WritePolicy};
use tokit_compiler::{check, native, run_with_capabilities};

fn temporary_directory() -> PathBuf {
    let nonce = common::nonce();
    let directory =
        std::env::temp_dir().join(format!("tokit-binary-{}-{nonce}", std::process::id()));
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

#[test]
fn binary_policies_keep_grants_and_preserve_invalid_utf8() {
    let directory = temporary_directory();
    let input = directory.join("input.bin");
    let output = directory.join("output.bin");
    let outside = directory
        .parent()
        .unwrap()
        .join(format!("tokit-outside-{}.bin", std::process::id()));
    std::fs::write(&input, [0, 255, 254, 128, 42]).unwrap();
    let denied = ReadPolicy::from_root(None);
    assert_eq!(
        denied.read_bytes(&input.to_string_lossy()),
        Err(IoError::Denied)
    );
    let reader = ReadPolicy::from_root(Some(&directory));
    assert_eq!(
        reader.read_text(&input.to_string_lossy()),
        Err(IoError::InvalidUtf8)
    );
    assert_eq!(
        reader.read_bytes(&input.to_string_lossy()).unwrap(),
        [0, 255, 254, 128, 42]
    );
    assert_eq!(
        reader.read_bytes(&outside.to_string_lossy()),
        Err(IoError::Denied)
    );
    assert_eq!(
        reader.read_bytes(&directory.join("missing.bin").to_string_lossy()),
        Err(IoError::NotFound)
    );
    let writer = WritePolicy::from_root(Some(&directory));
    assert_eq!(
        WritePolicy::from_root(None).write_bytes(&output.to_string_lossy(), &[1]),
        Err(IoError::Denied)
    );
    writer
        .write_bytes(&output.to_string_lossy(), &[0, 255, 254, 128, 42])
        .unwrap();
    assert_eq!(std::fs::read(&output).unwrap(), [0, 255, 254, 128, 42]);
    assert_eq!(
        writer.write_bytes(&outside.to_string_lossy(), &[1]),
        Err(IoError::Denied)
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn binary_copy_agrees_in_interpreter_and_native() {
    let directory = temporary_directory();
    let input_dir = directory.join("input");
    let output_dir = directory.join("output");
    std::fs::create_dir(&input_dir).unwrap();
    std::fs::create_dir(&output_dir).unwrap();
    let input = input_dir.join("source.bin");
    let output = output_dir.join("copy.bin");
    let payload = [0, 255, 254, 128, 42, 0];
    std::fs::write(&input, payload).unwrap();
    let source = format!(
        "fn main()->Result<Unit,IoError>{{let data:Bytes=read_bytes({})?;write_bytes({},data)}}",
        literal(&input),
        literal(&output)
    );
    assert_eq!(
        run_with_capabilities(&source, None, None, &[])
            .unwrap()
            .to_string(),
        "Err(IoError::Denied)"
    );
    assert_eq!(
        run_with_capabilities(&source, Some(&input_dir), None, &[])
            .unwrap()
            .to_string(),
        "Err(IoError::Denied)"
    );
    assert_eq!(
        run_with_capabilities(&source, Some(&input_dir), Some(&output_dir), &[])
            .unwrap()
            .to_string(),
        "Ok(())"
    );
    assert_eq!(std::fs::read(&output).unwrap(), payload);
    std::fs::remove_file(&output).unwrap();
    let example = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("examples/binary_copy.tok"),
    )
    .unwrap();
    let args = [input.display().to_string(), output.display().to_string()];
    assert_eq!(
        run_with_capabilities(&example, Some(&input_dir), Some(&output_dir), &args)
            .unwrap()
            .to_string(),
        "Ok(())"
    );
    assert_eq!(std::fs::read(&output).unwrap(), payload);
    std::fs::remove_file(&output).unwrap();
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        std::fs::remove_dir_all(directory).unwrap();
        return;
    }
    let executable = directory.join(format!("copy{}", std::env::consts::EXE_SUFFIX));
    native::build(&check(&source).unwrap(), &source, &executable).unwrap();
    let denied = Command::new(&executable).output().unwrap();
    assert!(denied.status.success());
    assert_eq!(
        String::from_utf8(denied.stdout).unwrap().trim(),
        "Err(IoError::Denied)"
    );
    let granted = Command::new(&executable)
        .args([
            "--allow-read",
            input_dir.to_str().unwrap(),
            "--allow-write",
            output_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        granted.status.success(),
        "{}",
        String::from_utf8_lossy(&granted.stderr)
    );
    assert_eq!(String::from_utf8(granted.stdout).unwrap().trim(), "Ok(())");
    assert_eq!(std::fs::read(&output).unwrap(), payload);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn binary_builtins_are_typed_reserved_and_effectful() {
    for (source, code) in [
        ("fn main()->Result<Bytes,IoError>{read_bytes(1)}", "E102"),
        (
            "fn main()->Result<Unit,IoError>{write_bytes(\"x\",[1])}",
            "E102",
        ),
        (
            "fn read_bytes(path:String)->Bytes{utf8_encode(path)} fn main()->i32{0}",
            "E106",
        ),
        (
            "fn f()->Result<Bytes,IoError>{read_bytes(\"x\")} fn main()->Task<Result<Bytes,IoError>>{spawn f()}",
            "E117",
        ),
        (
            "fn f()->Result<Unit,IoError>{write_bytes(\"x\",utf8_encode(\"y\"))} fn main()->Task<Result<Unit,IoError>>{spawn f()}",
            "E117",
        ),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}
