//! Hashes and encodings for the reference interpreter; native programs embed
//! the same source, so both backends produce identical bytes.
#![allow(clippy::all)]

include!("native_runtime/crypto.rs.txt");
