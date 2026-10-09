use tokit_compiler::{check, format, lexer};

#[test]
fn malformed_source_never_panics_during_lex_check_or_format() {
    let alphabet = [
        "a", "0", " ", "\n", "{", "}", "[", "]", "(", ")", "\"", "'", "/", "*", "=", "-", ":", ";",
        "?", "é", "😀",
    ];
    let mut state = 0x42a1_6a37_87dd_581bu64;
    for case in 0..512 {
        let mut source = String::new();
        for _ in 0..(case % 63 + 1) {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            source.push_str(alphabet[(state as usize) % alphabet.len()]);
        }
        let outcome = std::panic::catch_unwind(|| {
            let _ = lexer::lex(&source);
            let _ = check(&source);
            let _ = format::format(&source);
        });
        assert!(outcome.is_ok(), "case {case} panicked for {source:?}");
    }
}
