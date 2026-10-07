mod common;

use std::process::Command;

use tokit_compiler::{check, native, run};

fn native_result(source: &str) -> (bool, String) {
    let binary = std::env::temp_dir().join(format!(
        "tokit-arrays-{}-{}{}",
        std::process::id(),
        common::nonce(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&check(source).unwrap(), source, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    std::fs::remove_file(binary).unwrap();
    let stream = if output.status.success() {
        output.stdout
    } else {
        output.stderr
    };
    (
        output.status.success(),
        String::from_utf8(stream).unwrap().trim().to_owned(),
    )
}

#[test]
fn array_builtins_agree_between_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for (source, expected) in [
        (
            "main()->I{var t=0;for i in range(0,10){if i==3{continue;}if i==8{break;}t=t+i;}t}",
            "25",
        ),
        (
            "main()->[[I]]{[range(2,5),range(5,2),range(-2,0)]}",
            "[[2,3,4],[],[-2,-1]]",
        ),
        // Range bounds are evaluated once, left to right, before the loop.
        (
            "main()->[I]{var n=2;var seen:[I]=[];for i in range(0,{n=n+1;n}){n=0;seen.push(i);}seen}",
            "[0,1,2]",
        ),
        ("main()->[I]{[3,1]+[2]+[]}", "[3,1,2]"),
        ("main()->[I]{let xs:[I]=[];xs+xs}", "[]"),
        (
            "main()->[[I]]{let xs=[3,1,2];[sort(xs),reverse(xs),slice(xs,1,3),slice(xs,3,3),xs]}",
            "[[1,2,3],[2,1,3],[1,2],[],[3,1,2]]",
        ),
        (
            "main()->[F]{sort([2.5,0.0,-0.0,-1.0,1.0/0.0])}",
            "[-1.0,-0.0,0.0,2.5,inf]",
        ),
        (
            r#"main()->[String]{sort(["b","a","B","ä"])}"#,
            r#"["B","a","b","ä"]"#,
        ),
        (
            "main()->[bool]{[contains([1,2],2),contains([1,2],5),contains([0.0/0.0],0.0/0.0),contains([0.0],-0.0),contains([\"a\"],\"a\")]}",
            "[true,false,false,true,true]",
        ),
        (
            "total(n:I)->I{var t=0;for i in range(0,n){t=t+i;}t} main()->Result<I,TaskError>{join(spawn total(5))}",
            "Ok(10)",
        ),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
        if native_available {
            assert_eq!(
                native_result(source),
                (true, expected.to_owned()),
                "{source}"
            );
        }
    }
}

#[test]
fn invalid_array_operations_are_rejected() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for source in [
        "main()->[I]{slice([1],0,2)}",
        "main()->[I]{slice([1],1,0)}",
        "main()->[I]{slice([1],-1,1)}",
    ] {
        assert_eq!(run(source).unwrap_err().code, "E205", "{source}");
        if native_available {
            let (success, stderr) = native_result(source);
            assert!(!success && stderr.starts_with("E205"), "{source}: {stderr}");
        }
    }
    for (source, code) in [
        ("main()->[[I]]{sort([[1]])}", "E104"),
        ("main()->bool{let f=|x:I|x;contains([f],f)}", "E104"),
        ("main()->bool{contains([1],\"a\")}", "E102"),
        ("main()->[I]{[1]+[\"a\"]}", "E104"),
        ("main()->[I]{range(0,1i64)}", "E102"),
        ("fn range(a:I)->I{a} main()->I{1}", "E106"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}
