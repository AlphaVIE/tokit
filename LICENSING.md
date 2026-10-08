# Licensing evaluation

The brief asks for an open-source license suitable for broad compiler and
ecosystem adoption, evaluating MIT, Apache 2.0, and a dual license, with the
reasoning documented before the final selection. The owner selected the recommendation below: Tokit is licensed under
`MIT OR Apache-2.0` ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).

## What needs a license

| Part | Who uses it | Concern |
| --- | --- | --- |
| Compiler and tools (`compiler/`, `editors/`) | users, distributors, IDE vendors | patents, attribution |
| Runtime code embedded in every native binary (`compiler/src/native_runtime/*.rs.txt`) | every Tokit program | must not impose obligations on users' programs |
| Registry packages (`packages/`) | copied into users' projects | same as the runtime |
| Specifications and research (`spec/`, `research/`) | other implementers, model trainers | reuse in documents and datasets |
| Fine-tuning dataset (`research/finetune/data`) | model training | permissive terms for training and redistribution |

## Options

| | MIT | Apache-2.0 | MIT OR Apache-2.0 |
| --- | --- | --- | --- |
| Permissive, commercial use | yes | yes | yes |
| Explicit patent grant and patent retaliation | no | yes | yes (via Apache option) |
| Compatible with GPLv2 projects | yes | no | yes (via MIT option) |
| Notice burden | keep copyright notice | notice + state changes + NOTICE file | recipient chooses |
| Ecosystem precedent | many small tools | Swift, Kotlin, TypeScript, Android | Rust, Cargo, most Rust crates |

Copyleft licenses (GPL, MPL) were not shortlisted: the runtime is compiled
into every user program, so a copyleft runtime would either burden all Tokit
programs or need a separate runtime exception, which slows adoption.

## Recommendation

**Dual license `MIT OR Apache-2.0`** for code, packages, runtime, and the
dataset, with the runtime and generated code explicitly free of obligations
for compiled programs (the permissive licenses already allow this; a sentence
in the README removes doubt). Reasons:

1. It matches Rust's ecosystem, which Tokit's toolchain and native backend
   build on, so contributors and companies already accept it.
2. The Apache option gives users a patent grant, which matters for a compiler
   that companies may embed; the MIT option keeps GPLv2 compatibility.
3. Both are understood by automated license scanners, which matters for a
   language meant to be adopted through AI coding tools and package registries.

Specifications could additionally be offered under CC BY 4.0 if they are to
be republished as standalone documents; this is optional.

## Steps after the owner's decision

1. Add `LICENSE-MIT` and `LICENSE-APACHE` (or the chosen files) at the root.
2. Set `license = "MIT OR Apache-2.0"` in `Cargo.toml`, `packages/*/tok.toml`
   (once the manifest has the field), and `editors/vscode/package.json`.
3. State in the README that contributions are accepted under the same terms.
