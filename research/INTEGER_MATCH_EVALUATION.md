# Experimental integer match: small syntax check

The `i32` match syntax is a candidate in the experimental subset, not a final grammar decision. This check compares two equivalent one-function classifiers, excluding `main` and whitespace:

```tok
fn classify(n:i32)->String{match n{0=>"zero",1=>"one",_=>"other"}}
```

```tok
fn classify(n:i32)->String{if n==0{"zero"}else{if n==1{"one"}else{"other"}}}
```

Using locally installed `tiktoken` with `.venv/Scripts/python.exe`, the `match` form is 66 UTF-8 bytes and 23 tokens under each of `cl100k_base` and `o200k_base`; the nested `if` form is 76 bytes and 29 tokens under each. This is a source-only measurement of one example, not evidence of a general token or generation advantage. Interpreter and native parity tests cover the new patterns. The parser adds two pattern kinds, while the checker requires a final wildcard for the open `i32` domain and rejects duplicate or unreachable arms.
