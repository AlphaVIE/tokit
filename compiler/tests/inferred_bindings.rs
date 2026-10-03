use std::process::Command;

use tokit_compiler::{check, native, run};

#[test]
fn inferred_bindings_keep_mutability_and_run_in_both_backends() {
    let source = "fn main()->i32{let start=20;var total=start;total=total+1;let pair=[total,21];pair[0]+pair[1]}";
    assert_eq!(run(source).unwrap().to_string(), "42");
    if Command::new("rustc").arg("--version").output().is_ok() {
        let directory = std::env::temp_dir();
        let output = directory.join(format!(
            "tokit-inferred-{}-{}{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            std::env::consts::EXE_SUFFIX
        ));
        native::build(&check(source).unwrap(), source, &output).unwrap();
        let result = Command::new(&output).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), "42");
        std::fs::remove_file(output).unwrap();
    } else {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
}

#[test]
fn ambiguous_values_still_need_an_annotation() {
    for source in [
        "fn main()->[i32]{let xs=[];xs}",
        "fn main()->Option<i32>{let value=None;value}",
        "fn main()->Result<i32,i32>{let value=Ok(1);value}",
    ] {
        assert_eq!(check(source).unwrap_err().code, "E115", "{source}");
    }
    assert_eq!(
        check("fn main()->i32{var value=1;value=true;value}")
            .unwrap_err()
            .code,
        "E102"
    );
}
