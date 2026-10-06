use std::process::Command;

use tokit_compiler::{check, format, native, run};

fn native_output(source: &str) -> String {
    let binary = std::env::temp_dir().join(format!(
        "tokit-maps-{}-{}{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&check(source).unwrap(), source, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    std::fs::remove_file(binary).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn maps_agree_between_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for (source, expected) in [
        (
            "count(words:[String])->Map<String,I>{var m:Map<String,I>=Map();for w in words{m[w]=get_or(m,w,0)+1;}m} main()->Map<String,I>{count(split(\"b a c a b a\",\" \"))}",
            r#"{"a":3,"b":2,"c":1}"#,
        ),
        (
            "main()->[[I]]{var m:Map<I,I>=Map();m[3]=30;m[-1]=10;m[3]=31;let copy=m;m[0]=0;[[len(m),len(copy)],keys(m),values(copy)]}",
            "[[3,2],[-1,0,3],[10,31]]",
        ),
        (
            "main()->[Option<String>]{var m:Map<L,String>=Map();m[5000000000i64]=\"big\";[get(m,5000000000i64),get(m,1i64)]}",
            r#"[Some("big"),None]"#,
        ),
        (
            "main()->[bool]{var m:Map<bool,I>=Map();m[true]=1;[contains(m,true),contains(m,false),len(remove(m,true))==0,len(m)==1]}",
            "[true,false,true,true]",
        ),
        (
            "struct S{m:Map<String,[I]>} main()->S{var s=S(Map());s.m[\"x\"]=[1];s.m[\"x\"]=[1,2];s}",
            r#"S(m:{"x":[1,2]})"#,
        ),
        (
            "total(m:Map<String,I>)->I{var t=0;for v in values(m){t=t+v;}t} main()->Result<I,TaskError>{var m:Map<String,I>=Map();m[\"a\"]=2;m[\"b\"]=3;join(spawn total(m))}",
            "Ok(5)",
        ),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
        if native_available {
            assert_eq!(native_output(source), expected, "{source}");
        }
    }
}

#[test]
fn maps_are_typed() {
    for (source, code) in [
        ("main()->I{var m:Map<F,I>=Map();1}", "E103"),
        ("main()->I{var m:Map<[I],I>=Map();1}", "E103"),
        ("main()->I{var m:Map<I>=Map();1}", "E103"),
        ("main()->I{let m=Map();1}", "E115"),
        ("main()->I{var m:Map<String,I>=Map();m[1]=2;1}", "E102"),
        (
            "main()->I{var m:Map<String,I>=Map();m[\"a\"]=\"b\";1}",
            "E102",
        ),
        (
            "main()->I{var m:Map<String,I>=Map();let x=m[\"a\"];1}",
            "E110",
        ),
        (
            "struct P{x:I} main()->I{var m:Map<String,P>=Map();m[\"a\"].x=1;1}",
            "E110",
        ),
        ("main()->I{let m:Map<String,I>=Map(1);1}", "E105"),
        (
            "main()->bool{let m:Map<String,I>=Map();contains(m,1)}",
            "E102",
        ),
        ("main()->bool{let m:Map<String,I>=Map();m==m}", "E104"),
        ("struct Map{x:I} main()->I{1}", "E106"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

#[test]
fn closing_angle_may_touch_an_equals_sign() {
    for source in [
        "main()->I{let x:Option<I>=None;1}",
        "main()->I{let x:Option<Option<I>>=None;1}",
        "main()->I{var m:Map<String,[I]>=Map();1}",
    ] {
        assert_eq!(run(source).unwrap().to_string(), "1", "{source}");
        let formatted = format::format(source).unwrap();
        assert_eq!(run(&formatted).unwrap().to_string(), "1", "{source}");
    }
}
