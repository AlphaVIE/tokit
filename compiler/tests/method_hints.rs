//! Method-call syntax from other languages gets a repair hint (E113).

use tokit_compiler::check;

#[test]
fn method_calls_suggest_tokit_spelling() {
    for (source, message) in [
        (
            "main()->I{let xs=[1,2];xs.len()}",
            "[i32] has no fields; len is a function, not a method: pass the value first, as in len(x)",
        ),
        (
            "main()->Unit{var xs=[1];match 1{_=>xs.push(2)}}",
            "[i32] has no fields; push is a statement: write `xs.push(v);`, or `{xs.push(v);}` as a match arm",
        ),
        (
            "main()->String{let n=5;n.to_string()}",
            "i32 has no fields; call String(x)",
        ),
        (
            "main()->I{let s=\"ab\";s.length}",
            "String has no fields; call len(x)",
        ),
        (
            "struct P{x:I} main()->I{let p=P(1);p.trim()}",
            "unknown field trim on P; trim is a function, not a method: pass the value first, as in trim(x)",
        ),
        (
            "struct P{x:I} main()->I{let p=P(1);p.y}",
            "unknown field y on P",
        ),
        (
            "main()->I{let s=\"a\";s.foo}",
            "String has no fields; field access requires a record",
        ),
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(
            (error.code, error.message.as_str()),
            ("E113", message),
            "{source}"
        );
    }
}
