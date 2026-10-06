use std::path::PathBuf;
use std::process::Command;

use tokit_compiler::{check, native};

const SOURCE: &str = "main()->[String]{let start=clock_ns();sleep_ms(30i64);let took=clock_ns()-start;[match env(\"TOKIT_ENV_TIME_VAR\"){Some(v)=>v,None=>\"unset\"},String(now_ms()>1700000000000i64),String(took>=25000000i64),match env(\"TOKIT_ENV_TIME_MISSING\"){Some(v)=>v,None=>\"none\"}]}";
const EXPECTED: &str = r#"["hello","true","true","none"]"#;

fn source_file() -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "tokit-env-time-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("main.tok");
    std::fs::write(&path, SOURCE).unwrap();
    path
}

#[test]
fn environment_and_clocks_agree_between_backends() {
    let path = source_file();
    let interpreted = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("run")
        .arg(&path)
        .env("TOKIT_ENV_TIME_VAR", "hello")
        .env_remove("TOKIT_ENV_TIME_MISSING")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&interpreted.stdout).trim(),
        EXPECTED
    );
    if Command::new("rustc").arg("--version").output().is_ok() {
        let binary = path.with_extension(std::env::consts::EXE_EXTENSION);
        native::build(&check(SOURCE).unwrap(), SOURCE, &binary).unwrap();
        let compiled = Command::new(&binary)
            .env("TOKIT_ENV_TIME_VAR", "hello")
            .env_remove("TOKIT_ENV_TIME_MISSING")
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&compiled.stdout).trim(), EXPECTED);
    }
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn environment_and_clock_reads_are_effects() {
    for source in [
        "f()->Option<String>{env(\"X\")} main()->I{let t=spawn f();1}",
        "f()->L{now_ms()} main()->I{let t=spawn f();1}",
        "f()->L{clock_ns()} main()->I{let t=spawn f();1}",
        "f()->I{sleep_ms(1i64);1} main()->I{let t=spawn f();1}",
    ] {
        assert_eq!(check(source).unwrap_err().code, "E117", "{source}");
    }
    assert_eq!(check("main()->I{sleep_ms(1);1}").unwrap_err().code, "E102");
}
