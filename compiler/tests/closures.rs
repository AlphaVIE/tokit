mod common;

use std::process::Command;

use tokit_compiler::{check, native, run};

fn native_output(source: &str) -> String {
    let binary = std::env::temp_dir().join(format!(
        "tokit-closures-{}-{}{}",
        std::process::id(),
        common::nonce(),
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
fn closures_agree_between_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for (source, expected) in [
        // Captures are copies taken when the lambda is created.
        ("main()->I{var n=1;let f=|x:I|x+n;n=5;f(0)+n}", "6"),
        (
            "main()->[[I]]{let xs=[3,1,2];let k=10;[map(xs,|x|x*k),filter(xs,|x|x>1),sort_by(xs,|x|0-x)]}",
            "[[30,10,20],[3,2],[3,2,1]]",
        ),
        (
            "main()->[bool]{let xs=[1,2,3];[any(xs,|x|x>2),all(xs,|x|x>2),any([],|x:I|true),all([],|x:I|false)]}",
            "[true,false,false,true]",
        ),
        (
            "main()->String{fold([\"a\",\"b\"],\">\",|acc,s|acc+s)}",
            "\">ab\"",
        ),
        (
            "adder(n:I)->(I)->I{|x:I|x+n} main()->[I]{let add=adder(3);map([1,2],add)}",
            "[4,5]",
        ),
        (
            "compose(f:(I)->I,g:(I)->I)->(I)->I{|x:I|g(f(x))} main()->I{let h=compose(|x|x+1,|x|x*10);h(2)}",
            "30",
        ),
        (
            "struct Rule{name:String,check:(String)->bool} main()->[String]{let rules=[Rule(\"short\",|s|len(chars(s))<3),Rule(\"has_a\",|s|contains(s,\"a\"))];map(filter(rules,|r|r.check(\"ab\")),|r|r.name)}",
            r#"["short","has_a"]"#,
        ),
        (
            "main()->I{let fs=[|x:I|x+1,|x:I|x*2];fs[1](fs[0](4))}",
            "10",
        ),
        // A captured array parameter stays readable after the call that lent it.
        (
            "scale(xs:[I],k:I)->[I]{map(xs,|x|x*k+len(xs))} main()->[I]{let xs=[1,2];scale(xs,10)+xs}",
            "[12,22,1,2]",
        ),
        (
            "main()->[F]{sort_by([2.5,-1.0,0.0],|x|x)}",
            "[-1.0,0.0,2.5]",
        ),
        ("main()->(I)->I{|x:I|x}", "<fn>"),
        // A declared function of the same name wins over a local value.
        ("g(x:I)->I{x+100} main()->I{let g=|x:I|x;g(1)}", "101"),
        (
            "main()->I{let i32=|x:I|x;match i32(5i64){Some(v)=>v,None=>0}}",
            "5",
        ),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
        if native_available {
            assert_eq!(native_output(source), expected, "{source}");
        }
    }
}

#[test]
fn sequential_closure_calls_release_their_depth() {
    let source = "main()->I{let f=|x:I|x;var t=0;for i in range(0,20000){t=t+f(1);}t}";
    assert_eq!(run(source).unwrap().to_string(), "20000");
}

#[test]
fn closures_are_checked() {
    for (source, code) in [
        ("main()->I{var n=1;let f=|x:I|{n=2;x};f(1)}", "E109"),
        ("f()->I{let g=|x:I|{return x;};g(1)} main()->I{f()}", "E111"),
        (
            "f()->Option<I>{let g=|x:Option<I>|x?;Some(g(None))} main()->I{1}",
            "E111",
        ),
        ("main()->I{while true{let g=|x:I|{break;};}1}", "E002"),
        ("main()->I{let f=|x|x;1}", "E115"),
        ("main()->I{let f=|x:I|x;f(1,2)}", "E105"),
        ("main()->I{let f=1;f(1)}", "E101"),
        ("main()->bool{let f=|x:I|x;f==f}", "E104"),
        ("main()->I{let r=1;r.x(1)}", "E113"),
        ("main()->[[I]]{sort_by([[1]],|x|x)}", "E104"),
        ("main()->[I]{map([1],|x|\"a\")+[1]}", "E104"),
        ("main()->I{let f:(I)->I=|x:String|1;1}", "E102"),
        ("main()->I{let f=|x:I,x:I|x;1}", "E106"),
        // Tasks cannot receive or call function values.
        (
            "run(f:(I)->I)->I{f(1)} main()->I{let t=spawn run(|x|x);1}",
            "E117",
        ),
        (
            "struct Job{f:(I)->I} run(j:Job)->I{1} main()->I{let t=spawn run(Job(|x|x));1}",
            "E117",
        ),
        (
            "struct Job{f:(I)->I} run(j:Job)->I{j.f(1)} main()->I{1} go(j:Job)->Task<I>{spawn run(j)}",
            "E117",
        ),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
    // Pure lambdas inside a spawned function are fine.
    assert!(check("sum(xs:[I])->I{fold(xs,0,|t,x|t+x)} main()->Result<I,TaskError>{join(spawn sum([1,2]))}").is_ok());
}

#[test]
fn local_function_values_resolve_inside_imported_modules() {
    let directory = std::env::temp_dir().join(format!(
        "tokit-closure-modules-{}-{}",
        std::process::id(),
        common::nonce()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("util.tok"),
        "helper(x:I)->I{x+1} pub twice(f:(I)->I,x:I)->I{let g=f;g(helper(f(x)))}",
    )
    .unwrap();
    std::fs::write(
        directory.join("main.tok"),
        "import util=\"util.tok\"; main()->I{util::twice(|x|x*2,5)}",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("run")
        .arg(directory.join("main.tok"))
        .output()
        .unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "22",
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
