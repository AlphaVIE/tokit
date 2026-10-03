# Versioned benchmark snapshot

This folder captures one comparable cycle-sum workload in Tokit, JavaScript, TypeScript, Go, C++, C#, Python, and Rust.

All implementations follow the same contract:

- read exactly one command-line argument
- parse it as an integer iteration count
- add the repeating cycle `1, 2, 3`
- print `Ok(<sum>)`

Tokit should remain the shortest source representation for the contract because it keeps the parse/loop/result path compact and avoids extra wrapper syntax. Python is usually close in source length, while JavaScript, TypeScript, and Go tend to add some glue around argument parsing and output. C++, C#, and Rust are typically longer in source because they need more explicit imports, types, or boilerplate, but they are the native-runtime baseline for launch and steady-state execution.

Runtime expectation is split in two parts: source length and process runtime. Source length is already visible in the files here; runtime should be measured separately on the same host and toolchain. For a tiny arithmetic loop like this, interpreter startup and process launch will dominate JavaScript, TypeScript, and Python more than the loop itself, while compiled Go, C++, C#, Rust, and Tokit-native builds should track much closer to the actual loop cost.

This snapshot is meant as a comparative fixture, not as a general performance claim.

Future snapshots can increase complexity in small steps instead of changing the workload all at once. A sensible progression is to keep this cycle-sum core, then add one extra dimension per version, such as array access, checked error handling, file I/O, or a second function call path. That keeps the source comparison stable while making runtime and translation cost more representative over time.
