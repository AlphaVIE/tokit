use tokit_compiler::run_with_runtime_args;

const ARRAY_CYCLE: &str = include_str!("../../benchmarks_version_04102026_005141/array_cycle.tok");
const POINTER_CHASE: &str =
    include_str!("../../benchmarks_version_04102026_213748/pointer_chase.tok");
const GENERATED_CHASE: &str =
    include_str!("../../benchmarks_version_04102026_220333/generated_chase.tok");
const WIDE_SUM: &str = include_str!("../../benchmarks_version_05102026_022806/wide_sum.tok");

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

#[test]
fn wide_sum_uses_runtime_i64_values() {
    for (arguments, expected) in [
        (vec!["0", "3000000000", "4000000000"], "Ok(0)"),
        (
            vec!["7", "3000000000", "4000000000", "5000000000"],
            "Ok(27000000000)",
        ),
        (vec!["3", "5000000001", "6000000003"], "Ok(16000000005)"),
    ] {
        let arguments = arguments.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            run_with_runtime_args(WIDE_SUM, None, &arguments)
                .unwrap()
                .to_string(),
            expected
        );
    }
}
