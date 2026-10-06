mod common;

use std::path::PathBuf;
use std::process::Command;

use tokit_compiler::{check, native};

const SOURCE: &str = r#"struct Out{made:Result<Unit,IoError>,again:Result<Unit,IoError>,listed:Result<[String],IoError>,removed:Result<Unit,IoError>,missing:Result<Unit,IoError>,present:bool,gone:bool,outside:Result<[String],IoError>,escape:Result<Unit,IoError>,root_exists:bool}
main()->Out{let root=args()[0];let made=make_dir(root+"/sub");let again=make_dir(root+"/sub");let a=write_text(root+"/sub/a.txt","x");let b=write_text(root+"/b.txt","y");let listed=list_dir(root);let removed=remove_file(root+"/b.txt");Out(made,again,listed,removed,remove_file(root+"/b.txt"),exists(root+"/sub/a.txt"),exists(root+"/b.txt"),list_dir(root+"/.."),make_dir(root+"/../escape"),exists(root))}"#;
const EXPECTED: &str = r#"Out(made:Ok(()),again:Err(IoError::Other),listed:Ok(["b.txt","sub"]),removed:Ok(()),missing:Err(IoError::NotFound),present:true,gone:false,outside:Err(IoError::Denied),escape:Err(IoError::Denied),root_exists:true)"#;

fn fresh(label: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "tokit-fs-entries-{label}-{}-{}",
        std::process::id(),
        common::nonce()
    ));
    std::fs::create_dir_all(directory.join("granted")).unwrap();
    directory
}

#[test]
fn directory_operations_follow_grants_in_both_backends() {
    let directory = fresh("interpreter");
    let granted = directory.join("granted");
    let program = directory.join("main.tok");
    std::fs::write(&program, SOURCE).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("run")
        .arg("--allow-read")
        .arg(&granted)
        .arg("--allow-write")
        .arg(&granted)
        .arg(&program)
        .arg("--")
        .arg(&granted)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), EXPECTED);
    assert!(!directory.join("escape").exists());
    std::fs::remove_dir_all(&directory).unwrap();

    if Command::new("rustc").arg("--version").output().is_ok() {
        let directory = fresh("native");
        let granted = directory.join("granted");
        let binary = directory.join(format!("main{}", std::env::consts::EXE_SUFFIX));
        native::build(&check(SOURCE).unwrap(), SOURCE, &binary).unwrap();
        let output = Command::new(&binary)
            .arg("--allow-read")
            .arg(&granted)
            .arg("--allow-write")
            .arg(&granted)
            .arg("--")
            .arg(&granted)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), EXPECTED);
        assert!(!directory.join("escape").exists());
        std::fs::remove_dir_all(&directory).unwrap();
    }
}

#[test]
fn directory_operations_without_grants_are_denied() {
    let directory = fresh("denied");
    let granted = directory.join("granted");
    let source = "main()->[String]{let root=args()[0];[String(exists(root)),match list_dir(root){Ok(n)=>\"listed\",Err(e)=>\"denied\"},match make_dir(root+\"/x\"){Ok(u)=>\"made\",Err(e)=>\"denied\"}]}";
    let program = directory.join("main.tok");
    std::fs::write(&program, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("run")
        .arg(&program)
        .arg("--")
        .arg(&granted)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        r#"["false","denied","denied"]"#
    );
    std::fs::remove_dir_all(&directory).unwrap();
    for source in [
        "f()->bool{exists(\"a\")} main()->I{let t=spawn f();1}",
        "f()->Result<Unit,IoError>{remove_file(\"a\")} main()->I{let t=spawn f();1}",
    ] {
        assert_eq!(check(source).unwrap_err().code, "E117", "{source}");
    }
}
