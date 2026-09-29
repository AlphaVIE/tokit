# Token benchmark specification

## Unit of comparison

A fixture starts with a language-neutral behavior contract, inputs, outputs, error behavior, side effects, and concurrency semantics. Every implementation must satisfy the same contract. Review implementations before measuring. Record any standard-library capabilities assumed by each language; an unavailable capability must be implemented or the fixture excluded, never replaced with a stub.

For each source representation, measure UTF-8 bytes, Unicode scalar count, model tokens, and later parsed AST nodes, declarations, dependencies, and semantic operations. Report raw values and normalized ratios. Do not claim that the seed fixture is a complete program or executable compiler input.

## Tokenizers

Pin package versions, tokenizer model files/hashes, and encoding names. Use the exact tokenizer for each target model where available. A local command `python scripts/token_cost.py research/candidates` measures two OpenAI encodings when `tiktoken` is installed; it is an **initial instrumentation check**, not cross-family validation. Add at least one independently trained tokenizer family before ranking candidates. If a tokenizer cannot be redistributed, document its source, revision, hash, and loading instructions.

Count source only for the source-size view. Separately count all messages in model generation and repair sessions, including system/developer instructions used in the experiment, fixture description, source context, errors, and output. Keep input and output counts distinct. Log model version, sampling settings, seeds if supported, retries, timeouts, and pricing assumptions.

## Experimental protocol

1. Pre-register tasks, prompts, validators, and stopping rules. Randomize candidate order and hide notation names when possible.
2. Use training examples of comparable information content. Include zero-shot and small-example conditions.
3. Run repeated samples per candidate and task. Preserve every attempt, including invalid output and timeouts.
4. Compile/check and run the same tests. Distinguish parse, type, semantic, and runtime failures.
5. Offer identical feedback budgets for repair; record tokens and attempts until success or limit.
6. Repeat for small edits, cross-file changes, and error correction, not only initial generation.
7. Publish per-task results, distributions, uncertainty intervals, and failures. Do not hide unfavorable workloads.

The primary comparison is verified solutions per unit of total token budget, with runtime and safety as separate required dimensions. Use paired tasks and bootstrap confidence intervals for aggregate comparisons. A candidate is not accepted on a small nominal token win when its success rate or repair cost is materially worse.

## Fair baselines

Compare Tokit with Rust, Go, C, C++, Python, TypeScript, and Java using idiomatic code with equivalent behavior. Include imports, configuration, and glue needed for a runnable program. Report standard-library and third-party dependencies. Establish a no-dependency subset for core language comparisons and application fixtures for ecosystem comparisons. Pin compiler/interpreter versions and test on identical hardware.

## Future metrics

Once parsers exist, define an AST-node counting rule that excludes purely syntactic wrappers and record nodes per semantic operation. For performance benchmarks record wall and CPU time, peak resident memory, allocations, binary size, startup time, and build time. Separate warm and cold runs, set timeouts, and archive raw measurements and environment details.
