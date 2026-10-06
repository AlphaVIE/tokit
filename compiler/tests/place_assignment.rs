mod common;

use std::process::Command;

use tokit_compiler::{check, native, run};

fn native_result(source: &str) -> (bool, String) {
    let binary = std::env::temp_dir().join(format!(
        "tokit-place-{}-{}{}",
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
fn element_and_field_assignment_agree_between_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for (source, expected) in [
        (
            "main()->[I]{var xs=[1,2,3];let copy=xs;xs[1]=20;[xs[1],copy[1]]}",
            "[20,2]",
        ),
        (
            "main()->[[I]]{var m=[[0,0],[0,0]];var i=0;while i<2{m[i][i]=i+1;i=i+1;}m}",
            "[[1,0],[0,2]]",
        ),
        (
            "struct P{x:I,y:I} main()->P{var p=P(1,2);p.y=p.x+p.y;p.x=0;p}",
            "P(x:0,y:3)",
        ),
        (
            "struct C{on:bool} struct G{rows:[[C]]} main()->G{var g=G([[C(false)],[C(false)]]);g.rows[1][0].on=true;g}",
            "G(rows:[[C(on:false)],[C(on:true)]])",
        ),
        (
            "struct Box<T>{value:T} main()->Box<String>{var b=Box(\"a\");b.value=b.value+\"b\";b}",
            "Box(value:\"ab\")",
        ),
        (
            "main()->[[I]]{var d=utf8_encode(\"abc\");let before=d;d[2]=90;[bytes_to_i32(d),bytes_to_i32(before)]}",
            "[[97,98,90],[97,98,99]]",
        ),
        // Indices are evaluated before the assigned value.
        (
            "main()->[I]{var xs=[0,0,0];var i=0;xs[{i=i+1;i}]={i=i+10;i};xs[2]=i;xs}",
            "[0,11,11]",
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
fn invalid_assignment_targets_fail_in_both_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for (source, code) in [
        ("main()->I{var xs=[1];xs[3]=1;0}", "E205"),
        ("main()->I{var xs=[[1]];xs[0][-1]=1;0}", "E205"),
        ("main()->I{var d=utf8_encode(\"a\");d[0]=300;0}", "E207"),
        ("main()->I{var d=utf8_encode(\"a\");d[4]=300;0}", "E205"),
    ] {
        assert_eq!(run(source).unwrap_err().code, code, "{source}");
        if native_available {
            let (success, stderr) = native_result(source);
            assert!(!success && stderr.starts_with(code), "{source}: {stderr}");
        }
    }
    for (source, code) in [
        ("main()->I{let xs=[1];xs[0]=1;0}", "E109"),
        ("f(xs:[I])->I{xs[0]=1;0} main()->I{f([1])}", "E109"),
        ("main()->I{var x=1;x[0]=1;0}", "E110"),
        ("main()->I{var xs=[1];xs[0]=\"a\";0}", "E102"),
        ("main()->I{var xs=[1];xs[true]=1;0}", "E102"),
        ("struct P{x:I} main()->I{var p=P(1);p.z=1;0}", "E113"),
        ("main()->I{var x=1;x.y=1;0}", "E113"),
        ("f()->[I]{[1]} main()->I{f()[0]=1;0}", "E002"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}
