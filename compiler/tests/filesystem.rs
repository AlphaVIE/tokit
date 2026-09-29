use std::path::{Path, PathBuf};
use std::process::Command;

use tokit_compiler::{
    check,
    filesystem::{ReadError, ReadPolicy},
    run, run_with_read_root,
};

fn temporary_directory() -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("tokit-read-{}-{nonce}", std::process::id()));
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
fn read_capability_and_line_iteration_are_typed_and_scoped() {
    let directory = temporary_directory();
    let text_file = directory.join("sample.txt");
    std::fs::write(&text_file, "alpha\r\nbeta\n").unwrap();
    let source = format!(
        "fn main()->Result<i32,IoError>{{let text:String=read_text({})?;var count:i32=0;for line in lines(text){{count=count+1;}}Ok(count)}}",
        literal(&text_file)
    );
    assert_eq!(run(&source).unwrap().to_string(), "Err(IoError::Denied)");
    assert_eq!(
        run_with_read_root(&source, &directory).unwrap().to_string(),
        "Ok(2)"
    );
    let source_path = directory.join("count.tok");
    std::fs::write(&source_path, &source).unwrap();
    let cli = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("run")
        .arg("--allow-read")
        .arg(&directory)
        .arg(&source_path)
        .output()
        .unwrap();
    assert!(cli.status.success());
    assert_eq!(String::from_utf8(cli.stdout).unwrap().trim(), "Ok(2)");
    assert_eq!(
        run("fn main()->[String]{lines(\"a\\r\\nb\\n\")}")
            .unwrap()
            .to_string(),
        "[\"a\",\"b\"]"
    );
    for (source, code) in [
        ("fn main()->Result<String,IoError>{read_text(1)}", "E102"),
        ("fn main()->[String]{lines(true)}", "E102"),
        (
            "fn read_text(x:String)->String{x} fn main()->i32{0}",
            "E106",
        ),
        ("enum IoError{Other} fn main()->i32{0}", "E106"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn read_errors_do_not_escape_granted_path() {
    let directory = temporary_directory();
    let allowed = directory.join("allowed");
    std::fs::create_dir(&allowed).unwrap();
    let outside = directory.join("outside.txt");
    let invalid = allowed.join("invalid.bin");
    std::fs::write(&outside, "secret").unwrap();
    std::fs::write(&invalid, [0xff, 0xfe]).unwrap();
    let policy = ReadPolicy::from_root(Some(&allowed));
    assert_eq!(
        policy.read_text(&outside.to_string_lossy()),
        Err(ReadError::Denied)
    );
    assert_eq!(
        policy.read_text(&allowed.join("missing.txt").to_string_lossy()),
        Err(ReadError::NotFound)
    );
    assert_eq!(
        policy.read_text(&invalid.to_string_lossy()),
        Err(ReadError::InvalidUtf8)
    );
    assert_eq!(
        policy.read_text(&allowed.to_string_lossy()),
        Err(ReadError::Other)
    );
    let link = allowed.join("escape.txt");
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&outside, &link);
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file(&outside, &link);
    if linked.is_ok() {
        assert_eq!(
            policy.read_text(&link.to_string_lossy()),
            Err(ReadError::Denied)
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn native_read_grant_matches_interpreter() {
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        return;
    }
    let directory = temporary_directory();
    let file = directory.join("sample.txt");
    std::fs::write(&file, "a\nb\n").unwrap();
    let source = format!(
        "fn main()->Result<String,IoError>{{read_text({})}}",
        literal(&file)
    );
    let program = check(&source).unwrap();
    let executable = directory.join(format!("reader{}", std::env::consts::EXE_SUFFIX));
    tokit_compiler::native::build(&program, &source, &executable).unwrap();
    for args in [
        Vec::<String>::new(),
        vec!["--allow-read".into(), directory.display().to_string()],
    ] {
        let output = Command::new(&executable).args(&args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let expected = if args.is_empty() {
            run(&source).unwrap()
        } else {
            run_with_read_root(&source, &directory).unwrap()
        };
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            expected.to_string()
        );
    }
    let count_source = format!(
        "fn main()->Result<i32,IoError>{{let text:String=read_text({})?;var count:i32=0;for line in lines(text){{count=count+1;}}Ok(count)}}",
        literal(&file)
    );
    let count_program = check(&count_source).unwrap();
    let count_executable = directory.join(format!("count{}", std::env::consts::EXE_SUFFIX));
    tokit_compiler::native::build(&count_program, &count_source, &count_executable).unwrap();
    let output = Command::new(&count_executable)
        .arg("--allow-read")
        .arg(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "Ok(2)");
    let missing = directory.join("missing.txt");
    let match_source = format!(
        "fn main()->String{{match read_text({}){{Ok(text)=>text,Err(error)=>match error{{IoError::Denied=>\"denied\",IoError::NotFound=>\"missing\",IoError::InvalidUtf8=>\"utf8\",IoError::Other=>\"other\"}}}}}}",
        literal(&missing)
    );
    let match_program = check(&match_source).unwrap();
    let match_executable = directory.join(format!("match_io{}", std::env::consts::EXE_SUFFIX));
    tokit_compiler::native::build(&match_program, &match_source, &match_executable).unwrap();
    for (args, expected) in [
        (Vec::<String>::new(), "\"denied\""),
        (
            vec!["--allow-read".into(), directory.display().to_string()],
            "\"missing\"",
        ),
    ] {
        let output = Command::new(&match_executable)
            .args(&args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
    }
    std::fs::remove_dir_all(directory).unwrap();
}
