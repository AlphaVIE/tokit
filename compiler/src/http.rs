//! HTTP for the reference interpreter. The implementation is the text that
//! native programs embed, so both backends parse and format messages alike.
#![allow(non_camel_case_types, clippy::all)]

include!("native_runtime/http.rs.txt");
