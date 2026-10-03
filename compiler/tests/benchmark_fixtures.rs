use tokit_compiler::run_with_runtime_args;

const ARRAY_CYCLE: &str = include_str!("../../benchmarks_version_04102026_005141/array_cycle.tok");

#[test]
fn array_cycle_uses_runtime_values_and_matches_the_matrix_oracle() {
    for (arguments, expected) in [
        (vec!["0", "2", "5", "9"], "Ok(0)"),
        (vec!["7", "2", "5", "9"], "Ok(34)"),
        (vec!["8", "2", "5", "9"], "Ok(39)"),
    ] {
        let arguments = arguments.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            run_with_runtime_args(ARRAY_CYCLE, None, &arguments)
                .unwrap()
                .to_string(),
            expected
        );
    }
}
