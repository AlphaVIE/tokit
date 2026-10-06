//! Every `tokit` block in the LLM guide must check and run.

use std::path::Path;

#[test]
fn llm_guide_examples_check_and_run() {
    let guide =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../spec/LLM_GUIDE.md"))
            .unwrap();
    let blocks: Vec<&str> = guide
        .split("```tokit\n")
        .skip(1)
        .map(|rest| rest.split("```").next().unwrap())
        .collect();
    assert!(blocks.len() >= 5, "guide has too few examples");
    for block in blocks {
        tokit_compiler::check(block).unwrap_or_else(|error| panic!("{block}\n{error:?}"));
        tokit_compiler::run(block).unwrap_or_else(|error| panic!("{block}\n{error:?}"));
        let compact = tokit_compiler::format::compact_functions(block)
            .and_then(|text| tokit_compiler::format::compact_integer_types(&text))
            .and_then(|text| tokit_compiler::format::compact_blocks(&text))
            .unwrap();
        assert_eq!(compact, block, "guide examples must already be canonical");
    }
}
