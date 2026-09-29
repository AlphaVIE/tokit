use tokit_compiler::{check, run};

#[test]
fn signed_literals_cover_i32_min_and_negative_match_arms() {
    for (source, expected) in [
        ("fn main()->i32{-2147483648}", "-2147483648"),
        ("fn main()->i32{5--2}", "7"),
        ("fn main()->i32{match -1{-1=>7,_=>0}}", "7"),
        (
            "fn main()->i32{match -2147483648{-2147483648=>7,_=>0}}",
            "7",
        ),
        ("fn main()->i32{match 0{-1=>7,_=>0}}", "0"),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
    }
}

#[test]
fn signed_literals_reject_out_of_range_or_separated_digits() {
    for (source, code) in [
        ("fn main()->i32{-2147483649}", "E003"),
        ("fn main()->i32{match 0{-2147483649=>1,_=>2}}", "E003"),
        ("fn main()->i32{- 1}", "E002"),
        ("fn main()->i32{match 0{- 1=>1,_=>2}}", "E002"),
        ("fn main()->i32{--1}", "E002"),
        ("fn main()->i32{match 0{-0=>1,0=>2,_=>3}}", "E116"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}
