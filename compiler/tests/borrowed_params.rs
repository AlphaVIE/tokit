mod common;

use std::process::Command;

use tokit_compiler::{check, native, run};

fn native_output(source: &str) -> String {
    let binary = std::env::temp_dir().join(format!(
        "tokit-borrow-{}-{}{}",
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

/// Non-scalar parameters are lent to native callees; each use must still
/// observe value semantics and agree with the reference interpreter.
#[test]
fn lent_parameters_keep_value_semantics() {
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(
            std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(),
            Ok("1"),
            "rustc required by CI"
        );
        return;
    }
    let source = r#"
struct Point{x:I,y:I}
enum Shape{Dot,Line([Point])}
struct Box<T>{value:T}
sum(xs:[I])->I{var total=0;for x in xs{total=total+x;}total}
first(xs:[I])->I{xs[0]+len(xs)}
forward(xs:[I])->I{sum(xs)+first(xs)}
grow(xs:[I])->[I]{var copy=xs;copy.push(9);copy}
norm(p:Point)->I{p.x*p.x+p.y*p.y}
points(s:Shape)->I{match s{Shape::Dot=>0,Shape::Line(ps)=>len(ps)}}
byte(data:Bytes)->I{data[1]+len(data)}
generic<T>(items:[T],fallback:T)->T{if len(items)>0{items[0]}else{fallback}}
boxed<T>(b:Box<T>)->T{b.value}
pick(o:Option<String>)->String{match o{Some(v)=>v,None=>"none"}}
main()->[I]{
  let xs=[1,2,3];
  let grown=grow(xs);
  let task=spawn sum(grown);
  let total=match join(task){Ok(v)=>v,Err(e)=>0};
  [forward(xs),len(xs),len(grown),total,norm(Point(3,4)),points(Shape::Line([Point(1,2)])),byte(utf8_encode("hey")),generic(xs,0),generic([],7),boxed(Box(5)),len(utf8_encode(pick(Some("ab"))+pick(None)))]
}
"#;
    let expected = "[10,3,4,15,25,1,104,1,7,5,6]";
    assert_eq!(run(source).unwrap().to_string(), expected);
    assert_eq!(native_output(source), expected);
}
