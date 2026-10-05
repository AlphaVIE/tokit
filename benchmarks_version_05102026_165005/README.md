# Record field projection benchmark

The program constructs a record with a runtime-sized `[I]` field and a small
`I` field. It then reads the small field for a runtime-selected number of
iterations. The exact output is `Ok(7 * iterations)` for positive inputs that
fit in `I`. The array size must affect construction but not the returned
value. Example: `tok run record_field.tok -- 2000 4096` prints `Ok(14000)`.

This fixture isolates a potential unnecessary copy of the unused array during
`p.tag` projection. Measure both `tok run` and the native executable with
multiple array sizes, verify every result, and retain raw timed samples.
Whole-process times include startup and input construction; they do not
measure field access alone.
