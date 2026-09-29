use std::process::Command;
use tokit_compiler::{check, interpreter::Value, run};

#[test]
fn executes_functions_bindings_and_branches() {
    let source =
        "fn add(a:i32,b:i32)->i32{a+b} fn main()->i32{let x:i32=40;if x>0{add(x,2)}else{0}}";
    assert_eq!(run(source).unwrap(), Value::I32(42));
}

#[test]
fn utf8_strings_escape_concatenate_and_compare() {
    let source = r#"fn greet(x:String)->String{"Grüß, " + x + "\n"} fn main()->[String]{[greet("世界"),if "é"=="e"{"wrong"}else{"✓"}]}"#;
    assert_eq!(
        run(source).unwrap().to_string(),
        "[\"Grüß, 世界\\n\",\"✓\"]"
    );
    assert_eq!(
        check(r#"fn main()->String{"bad\q"}"#).unwrap_err().code,
        "E004"
    );
    assert_eq!(
        check("fn main()->String{\"unclosed}").unwrap_err().code,
        "E004"
    );
    assert_eq!(
        check(r#"fn main()->i32{"a"-"b"}"#).unwrap_err().code,
        "E104"
    );
}

#[test]
fn array_indexing_checks_type_and_bounds() {
    let source =
        "fn pick(xs:[String],i:i32)->String{xs[i]} fn main()->String{pick([\"a\",\"✓\"],1)}";
    assert_eq!(run(source).unwrap(), Value::String("✓".to_owned()));
    assert_eq!(
        run("fn main()->i32{[[1,2],[3,4]][1][0]}").unwrap(),
        Value::I32(3)
    );
    for source in [
        "fn main()->i32{[1][1]}",
        "fn main()->i32{[1][0-1]}",
        "fn main()->i32{let xs:[i32]=[];xs[0]}",
    ] {
        assert_eq!(run(source).unwrap_err().code, "E205", "{source}");
    }
    assert_eq!(check("fn main()->i32{1[0]}").unwrap_err().code, "E110");
    assert_eq!(check("fn main()->i32{[1][true]}").unwrap_err().code, "E102");
}

#[test]
fn records_construct_and_project_typed_fields() {
    let source = r#"struct Point{x:i32,y:i32} struct Label{point:Point,text:String} fn make(n:i32)->Label{Label(Point(n,n+1),"✓")} fn main()->i32{make(4).point.y}"#;
    assert_eq!(run(source).unwrap(), Value::I32(5));
    let source = r#"struct Pair{left:i32,right:String} fn main()->Pair{Pair(3,"x")}"#;
    assert_eq!(run(source).unwrap().to_string(), "Pair(left:3,right:\"x\")");
    assert_eq!(
        run("struct Empty{} fn main()->Empty{Empty()}")
            .unwrap()
            .to_string(),
        "Empty()"
    );
    let source = "struct Node{children:[Node]} fn main()->Node{Node([Node([])])}";
    assert_eq!(
        run(source).unwrap().to_string(),
        "Node(children:[Node(children:[])])"
    );
}

#[test]
fn invalid_records_are_rejected_before_execution() {
    for (source, code) in [
        ("struct Bad{x:Missing} fn main()->i32{0}", "E103"),
        ("fn main()->Missing{0}", "E103"),
        ("struct Bad{x:i32,x:bool} fn main()->i32{0}", "E106"),
        ("struct A{x:A} fn main()->i32{0}", "E112"),
        ("struct A{x:Result<A,i32>} fn main()->i32{0}", "E112"),
        ("struct A{x:B} struct B{x:A} fn main()->i32{0}", "E112"),
        ("struct A{x:i32} fn main()->A{A()}", "E105"),
        ("struct A{x:i32} fn main()->i32{A(true).x}", "E102"),
        ("struct A{x:i32} fn main()->i32{A(1).y}", "E113"),
        ("fn main()->i32{1.x}", "E113"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

#[test]
fn enum_variants_are_typed_error_values() {
    let source = "enum DivError{DivZero,Overflow} fn div(a:i32,b:i32)->Result<i32,DivError>{if b==0{Err(DivError::DivZero)}else{Ok(a/b)}} fn main()->Result<i32,DivError>{div(7,0)}";
    assert_eq!(run(source).unwrap().to_string(), "Err(DivError::DivZero)");
    let source = "enum Flag{On,Off} fn main()->[Flag]{[Flag::On,Flag::Off]}";
    assert_eq!(run(source).unwrap().to_string(), "[Flag::On,Flag::Off]");
}

#[test]
fn invalid_enum_variants_are_rejected() {
    for (source, code) in [
        ("enum E{A,A} fn main()->i32{0}", "E106"),
        ("enum E{A} fn main()->E{E::B}", "E114"),
        ("fn main()->i32{Missing::A;0}", "E103"),
        ("struct P{x:i32} fn main()->P{P::X}", "E114"),
        ("enum E{A} fn main()->E{E()}", "E114"),
        ("enum E{A} struct E{} fn main()->i32{0}", "E106"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

#[test]
fn executes_recursion_and_early_return() {
    let source =
        "fn fact(n:i32)->i32{if n<=1{return 1;}else{}; n*fact(n-1)} fn main()->i32{fact(5)}";
    assert_eq!(run(source).unwrap(), Value::I32(120));
}

#[test]
fn checks_before_running() {
    let cases = [
        ("fn main()->i32{true+1}", "E104"),
        ("fn main()->i32{missing}", "E101"),
        ("fn main()->i32{let x:bool=1;0}", "E102"),
        ("fn main()->i32{if 1{2}else{3}}", "E102"),
        ("fn f(x:i32)->i32{x} fn main()->i32{f()}", "E105"),
        ("fn main()->i32{0} fn main()->i32{1}", "E106"),
        ("fn main()->i32{2147483648}", "E003"),
        ("fn main()->i32{0", "E002"),
        ("fn main()->i32{@}", "E001"),
        ("fn main()->i32{return 1; 2}", "E107"),
    ];
    for (source, expected) in cases {
        assert_eq!(check(source).unwrap_err().code, expected, "{source}");
    }
}

#[test]
fn checked_arithmetic_reports_runtime_error() {
    assert_eq!(
        run("fn main()->i32{2147483647+1}").unwrap_err().code,
        "E201"
    );
    assert_eq!(run("fn main()->i32{1/0}").unwrap_err().code, "E201");
}

#[test]
fn run_requires_parameterless_main() {
    assert_eq!(run("fn other()->i32{1}").unwrap_err().code, "E203");
    assert_eq!(run("fn main(x:i32)->i32{x}").unwrap_err().code, "E203");
}

#[test]
fn machine_diagnostic_has_stable_span_and_escaping() {
    use tokit_compiler::{ast::Span, diagnostic::Diagnostic};
    let diagnostic = Diagnostic::new("E999", Span { start: 2, end: 4 }, "bad \"name\"\n");
    assert_eq!(
        diagnostic.json("a\né"),
        r#"{"code":"E999","span":{"start":2,"end":4},"line":2,"column":1,"message":"bad \"name\"\n"}"#
    );
}

#[test]
fn cli_emits_json_success() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["check", "--json", "examples/answer.tok"])
        .current_dir(workspace)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        r#"{"ok":true,"result":"ok"}"#
    );
}

#[test]
fn malformed_short_inputs_do_not_panic() {
    let alphabet = b"fn let if else return(){}:,+-*/=<>;0123abc \n";
    let mut state = 0x839bc62bu32;
    for _ in 0..2000 {
        let mut source = String::new();
        for _ in 0..48 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            source.push(alphabet[(state as usize) % alphabet.len()] as char);
        }
        assert!(
            std::panic::catch_unwind(|| check(&source)).is_ok(),
            "{source:?}"
        );
    }
}

#[test]
fn arrays_loops_and_mutation_execute() {
    let source = "fn sum(xs:[i32])->i32{var total:i32=0;for x in xs{total=total+x;}total} fn main()->i32{sum([3,4,5])}";
    assert_eq!(run(source).unwrap(), Value::I32(12));
    let source = "fn main()->[i32]{let xs:[i32]=[];xs}";
    assert_eq!(run(source).unwrap(), Value::Array(vec![]));
}

#[test]
fn return_inside_loop_exits_function() {
    let source = "fn main()->i32{for x in [1,2]{if x==2{return x;}else{};}0}";
    assert_eq!(run(source).unwrap(), Value::I32(2));
}

#[test]
fn invalid_arrays_and_mutation_are_rejected() {
    let cases = [
        ("fn main()->i32{let xs:[i32]=[1,true];0}", "E102"),
        ("fn main()->i32{let x:i32=1;x=2;x}", "E109"),
        ("fn main()->i32{var x:i32=1;x=true;x}", "E102"),
        ("fn main()->i32{for x in 1{}0}", "E110"),
        ("fn main()->i32{for x in [1]{}x}", "E101"),
    ];
    for (source, expected) in cases {
        assert_eq!(check(source).unwrap_err().code, expected, "{source}");
    }
}

#[test]
fn typed_results_and_propagation_execute() {
    let source = "fn divide(a:i32,b:i32)->Result<i32,i32>{if b==0{Err(1)}else{Ok(a/b)}} fn main()->Result<i32,i32>{let x:i32=divide(8,2)?;Ok(x+1)}";
    assert_eq!(run(source).unwrap(), Value::Ok(Box::new(Value::I32(5))));
    let source = "fn divide(a:i32,b:i32)->Result<i32,i32>{if b==0{Err(1)}else{Ok(a/b)}} fn main()->Result<i32,i32>{let x:i32=divide(8,0)?;Ok(x+1)}";
    assert_eq!(run(source).unwrap(), Value::Err(Box::new(Value::I32(1))));
}

#[test]
fn result_payloads_and_propagation_are_checked() {
    let cases = [
        ("fn main()->Result<i32,i32>{Ok(true)}", "E102"),
        ("fn main()->Result<i32,i32>{Err(true)}", "E102"),
        ("fn main()->i32{Ok(1)?}", "E111"),
        ("fn main()->Result<i32,i32>{1?}", "E111"),
        (
            "fn f()->Result<i32,bool>{Err(true)} fn main()->Result<i32,i32>{let x:i32=f()?;Ok(x)}",
            "E102",
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(check(source).unwrap_err().code, expected, "{source}");
    }
}

#[test]
fn propagation_crosses_loop_scope_and_result_arrays_unify() {
    let source = "fn div(a:i32,b:i32)->Result<i32,i32>{if b==0{Err(1)}else{Ok(a/b)}} fn main()->Result<i32,i32>{for x in [1,0]{let q:i32=div(8,x)?;}Ok(5)}";
    assert_eq!(run(source).unwrap(), Value::Err(Box::new(Value::I32(1))));
    let source = "fn main()->[Result<i32,i32>]{[Ok(1),Err(2)]}";
    assert_eq!(
        run(source).unwrap(),
        Value::Array(vec![
            Value::Ok(Box::new(Value::I32(1))),
            Value::Err(Box::new(Value::I32(2))),
        ])
    );
}
