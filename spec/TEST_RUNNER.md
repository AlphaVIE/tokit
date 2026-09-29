# Experimental test runner

`tok test [--allow-read <path>] file.tok` checks the whole file and discovers
functions whose names begin with `test_`. Tests run in declaration order in the
reference interpreter; a `main` function is unnecessary. A test must have no
parameters or type parameters and return `bool` or `Result<bool,E>`. `true` and
`Ok(true)` pass. `false`, `Ok(false)`, `Err(value)`, and runtime diagnostics
fail. Each test is invoked separately, and a failure does not stop later
tests. The command prints each result and a summary, then exits with status 1
if any test failed or if checking or discovery failed. No tests is an error.

The optional read grant has the same path policy as `tok run`. Without it,
`read_text` yields `IoError::Denied`. Test functions see an empty `args()` list.
See [tests.tok](../examples/tests.tok) for a runnable suite.

The `test_` convention and interpreter execution are provisional. This is a
single-file runner without fixtures, parallel scheduling, benchmarking,
coverage, or native test execution. The interpreter's current call-depth and
runtime limitations still apply.
