# Record field projection without a full value copy

Reading `p.tag` copied the complete `Payload` record in the reference
interpreter and the native bootstrap before selecting `tag`. The interpreter
now borrows a direct variable binding and clones only the selected field.
The native emitter projects the field from the binding and clones that field.
Computed record expressions still evaluate normally. Record assignment and
explicit copies retain value semantics.

The [versioned fixture](../benchmarks_version_05102026_165005/README.md)
constructs a record with a runtime-sized array and reads its small integer
field 2,000 times. Every warmup and timed run had to print `Ok(14000)`. The
[raw report](results/record-field-projection-2026-10-05-windows-gnu.json)
contains the fixture and binary hashes and eight alternating samples per
variant for each size. The baseline CLI was built from main commit `ad39ec1`;
both CLIs used Rust 1.98.1 GNU debug builds. Native executables used the
bootstrap's `opt-level=2` setting.

| Mode | Array elements | Before median ms | After median ms | Ratio |
| --- | ---: | ---: | ---: | ---: |
| Interpreter | 4,096 | 279.11 | 28.34 | 9.85× |
| Interpreter | 16,384 | 1,052.58 | 54.96 | 19.15× |
| Native | 4,096 | 12.35 | 11.61 | 1.06× |
| Native | 16,384 | 55.56 | 11.86 | 4.68× |

The fixture deliberately stresses an unused large field. Results are
whole-process times, including startup, parsing and checking for the
interpreter, and array construction for both modes. They do not establish a
general language speedup. The 4,096-element native difference is small
relative to process overhead.

```text
.venv/Scripts/python.exe scripts/benchmark_record_field.py --fixture benchmarks_version_05102026_165005/record_field.tok --before-tok .git/record-field-before-tok.exe --after-tok target/debug/tok.exe --before-native .git/record-field-before-native.exe --after-native .git/record-field-after-native.exe --output research/results/record-field-projection-2026-10-05-windows-gnu.json
```
