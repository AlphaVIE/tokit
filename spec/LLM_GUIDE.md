# Tokit guide for language models

This is a compact, complete reference for writing Tokit. It doubles as a
system prompt. Write the canonical compact form shown here; `tok compact`
produces it from any accepted spelling. Every code block below is checked
by `compiler/tests/llm_guide.rs`.

## Shape of a program

```tokit
// Declarations in any order; `main` is the entry point and its value is printed.
struct Point{x:I,y:I}
enum Shape{Dot,Circle(F)}
dist2(p:Point)->I{p.x*p.x+p.y*p.y}
main()->I{dist2(Point(3,4))}
```

- Functions: `name(param:Type,...)->Ret{body}`; the last expression is the result; `return x;` exits early.
- No `fn` keyword. Types: `I`=i32, `L`=i64, `F`=f64, `bool`, `String`, `Bytes`, `Unit`, `[T]`, `Option<T>`, `Result<T,E>`, `Map<K,V>`, `(A,B)->R`.
- `let` is immutable, `var` is mutable; types are inferred: `let n=1;` `var xs:[I]=[];` (annotate empty or ambiguous values).
- Parameters are immutable. Values are copied (no references, no null, no aliasing).
- Literals: `1`, `3000000000i64`, `1.5`, `1e3`, `-2`, `"text\n"`, `true`, `[1,2]`. No implicit numeric conversion: use `i64(n)`, `f64(n)`, `i32(x)->Option<I>`, `String(n)`.
- `pub` exports a declaration from a module: `import m="m.tok";` then `m::f(1)`.

## Control flow

```tokit
classify(n:I)->String{if n<0{"negative"}else if n==0{"zero"}else{"positive"}}
total(xs:[I])->I{var t=0;for x in xs{if x<0{continue;}t=t+x;}t}
count_down(n:I)->[I]{var out:[I]=[];var i=n;while i>0{out.push(i);i=i-1;}out}
main()->[String]{var r:[String]=[];for i in range(0,3){r.push(classify(i-1));}r}
```

- `if` without `else` only when the branch has type `Unit`. `if`, `match`, `for`, `while`, and `{}` statements need no trailing `;`.
- `for x in xs{}`, `for i in range(a,b){}` (half-open), `while cond{}`, `break;`, `continue;`.
- Operators: `+ - * / %` (integers are checked; overflow or division by zero is runtime error `E201`), `== != < <= > >=`, `&& || !`. Strings: `+` concatenates, `<` compares.

## Data

```tokit
enum Op{Add(I),Neg,Stop}
struct State{total:I,log:[String]}
step(s:State,op:Op)->State{match op{Op::Add(n)=>State(s.total+n,s.log+["add"]),Op::Neg=>State(0-s.total,s.log+["neg"]),Op::Stop=>s}}
main()->State{var s=State(0,[]);for op in [Op::Add(5),Op::Neg,Op::Add(2)]{s=step(s,op);}s.log[0]="first";s}
```

- Records: `Point(1,2)`, `p.x`; assignment through mutable bindings: `p.x=3;`, `xs[i]=v;`, `grid[y][x]=v;`.
- Enums with optional single payload; generic records/enums: `struct Box<T>{v:T}`, `enum Tree<T>{Leaf,Node([T])}`.
- `match` must be exhaustive. Patterns: `Ok(x)`, `Err(e)`, `Some(x)`, `None`, `Enum::V(x)`, `Enum::V`, `true`, literals `1`, `"add"`, `_`. Integer and string matches need a final `_`.

## Errors and options

```tokit
parse_pair(a:String,b:String)->Result<I,ParseError>{Ok(parse_i32(a)?+parse_i32(b)?)}
first_even(xs:[I])->Option<I>{for x in xs{if x%2==0{return Some(x);}}None}
main()->String{match parse_pair("4","x"){Ok(n)=>String(n),Err(ParseError::Invalid)=>"invalid",Err(e)=>"range"}}
```

- `==` compares structurally (records, enums, arrays, `Option`, `Result`): `x==None`, `p==P(1,2)`; maps and functions have no `==`.
- `?` propagates `Err` (in `Result` functions) or `None` (in `Option` functions).
- Patterns nest: `Ok(Some(Shape::Circle(r)))`, `Err(IoError::NotFound)`, literals inside (`Some(0)`, `Some("x")`); a bare name such as `other=>` binds the whole value. Arms must be exhaustive and reachable (`E116` names the missing case).

## Functions as values

```tokit
struct Person{name:String,age:I}
adder(n:I)->(I)->I{|x:I|x+n}
main()->[String]{let people=[Person("bo",30),Person("al",25)];let add=adder(1);map(filter(sort_by(people,|p|p.age),|p|add(p.age)>26),|p|p.name)}
```

- Lambdas `|x|expr`, `|a,b|expr`; parameter types come from context, else annotate `|x:I|`. Captures are copies; no `return`/`?`/`break` inside.
- Call a local function value `f(x)`; a field or element value `op.run(x)`, `fs[0](x)`.

## Built-in functions

- Arrays: `len`, `push` (statement `xs.push(v);`), `+`, `range(a,b)`, `sort`, `reverse`, `slice(xs,a,b)`, `contains(xs,v)`, `map`, `filter`, `any`, `all`, `fold(xs,init,|acc,x|...)`, `sort_by(xs,|x|key)`.
- Strings: `String(n)`, `chars`, `split(s,sep)`, `join(parts,sep)`, `trim`, `contains`, `starts_with`, `ends_with`, `replace(s,from,to)`, `lower`, `upper`, `lines`, `utf8_encode`, `utf8_decode_bytes`, `parse_i32`, `parse_i64`, `parse_f64` (each returns `Result<_,ParseError>`).
- Maps: `var m:Map<String,I>=Map();` `m[k]=v;` `get(m,k)->Option<V>`, `get_or(m,k,d)`, `contains(m,k)`, `keys`, `values`, `len`, `remove(m,k)`. Keys: `I`, `L`, `String`, `bool`; iteration order is sorted.
- Math: `abs`, `min`, `max`, `pow`, `sqrt`, `floor`, `ceil`, `round`, `exp`, `ln`, `sin`, `cos`, `tan`, `atan2`, `pi()`.
- Effects: `print(s)`, `read_line()->Option<String>`, `read_stdin()->Result<String,IoError>`, `args()->[String]`, `exit(code)`, `env(name)->Option<String>`, `now_ms()`, `clock_ns()`, `sleep_ms(ms)`.
- Files (need `--allow-read`/`--allow-write`): `read_text`, `write_text`, `read_bytes`, `write_bytes`, `list_dir`, `exists`, `make_dir`, `remove_file`.
- HTTP (needs `--allow-net`): `serve(addr,limit,|r|Response(200,Map(),"ok"))` (or `serve(addr,limit,workers,handler)` to answer in parallel), `http_request(method,url,headers,body)`; records `Request{method,path,query,headers,body}`, `Response{status,headers,body}`. Stateful servers loop themselves: `let l=listen(addr)?;` then `accept(l)`, `http_read(c)`, `http_write(c,res)`, `tcp_close(c)`.
- TCP (needs `--allow-net`): `tcp_connect(addr)->Result<Conn,IoError>`, `tcp_send(c,bytes)`, `tcp_recv(c,max)->Result<Bytes,IoError>` (empty at end of stream), `tcp_close(c)`. No TLS.
- Bytes and crypto: `sha256`, `sha1`, `md5`, `hmac_sha256(key,data)`, `pbkdf2_sha256(pw,salt,iterations)`, `base64_encode`, `base64_decode->Option<Bytes>`, `hex`, `random_bytes(n)`.
- Bits (no `|`/`&` operators): `bit_and`, `bit_or`, `bit_xor`, `bit_not`, `shl(x,n)`, `shr(x,n)` on `I` or `L`.
- Unit value: `()`, e.g. `Ok(())` in a `Result<Unit,E>` function.
- Tasks: `let t=spawn f(x);` then `join(t)->Result<T,TaskError>`; spawned functions must be pure.

## Workflow and diagnostics

1. `tok check --json file.tok` returns `{"ok":true}` or an error with `code`, `line`, `column`, `message`.
2. `tok run file.tok -- args`, `tok test file.tok`, `tok build file.tok -o app`, `tok compact --write file.tok`.
3. Packages: `tok new app`, `tok add json` (also `http`, `redis`, `postgres`, `websocket`; `tok search` lists them), then `import json="pkg:json";` and call `json.parse(text)`.
4. Inspect and polish: `tok lint file.tok` (unused names, `var` never changed, dead private functions), `tok expand file.tok` (readable layout), `tok explain --pseudo file.tok`, `tok doc file.tok`, `tok bench file.tok` (times `bench_*` functions), `tok repl`.

Frequent codes: `E002` syntax, `E101` unknown name, `E102` type mismatch (message shows expected and actual), `E104` invalid operands, `E105` wrong argument count, `E106` duplicate or reserved name, `E109` assigning an immutable binding, `E110` invalid indexing, `E113` unknown field, `E115` add a type annotation, `E116` non-exhaustive or invalid `match`, `E117` effect in a spawned function, `E201` integer overflow or division by zero at runtime, `E205` index out of bounds.

Fix diagnostics by changing the smallest region the message names; do not add casts the language lacks or methods other than `.push`. `E113` on `x.len()`-style calls says which function or statement to use instead.
