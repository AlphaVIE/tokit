use tokit_compiler::run_with_runtime_args;

const ARRAY_CYCLE: &str = include_str!("../../benchmarks_version_04102026_005141/array_cycle.tok");
const POINTER_CHASE: &str =
    include_str!("../../benchmarks_version_04102026_213748/pointer_chase.tok");
const GENERATED_CHASE: &str =
    include_str!("../../benchmarks_version_04102026_220333/generated_chase.tok");

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

#[test]
fn pointer_chase_uses_runtime_indices_and_matches_the_matrix_oracle() {
    for (arguments, expected) in [
        (vec!["0", "1", "2", "3", "0"], "Ok(0)"),
        (vec!["7", "1", "2", "3", "0"], "Ok(12)"),
        (vec!["8", "1", "2", "3", "0"], "Ok(12)"),
    ] {
        let arguments = arguments.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            run_with_runtime_args(POINTER_CHASE, None, &arguments)
                .unwrap()
                .to_string(),
            expected
        );
    }
}

#[test]
fn generated_chase_uses_runtime_size_and_stride() {
    for (arguments, expected) in [
        (vec!["7", "4", "1"], "Ok(3)"),
        (vec!["2", "4", "3"], "Ok(0)"),
        (vec!["0", "4", "1"], "Ok(0)"),
    ] {
        let arguments = arguments.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            run_with_runtime_args(GENERATED_CHASE, None, &arguments)
                .unwrap()
                .to_string(),
            expected
        );
    }
}
