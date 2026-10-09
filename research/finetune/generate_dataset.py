"""Build a verified Tokit instruction-tuning dataset.

Every sample comes from a parametrized task family that also computes the
expected result in Python. A sample is kept only when the canonical Tokit
program (after `tok compact` and `tok fmt`) checks and `tok run` prints
exactly that result. Repair samples mutate verified programs, record the
compiler's JSON diagnostic, and ask for the original program back.

Splits are made by family for the out-of-distribution test set, so no task
family seen in training appears there.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import subprocess
import tempfile
from collections import Counter
from collections.abc import Callable
from pathlib import Path

SYSTEM = (
    "You write Tokit, a token-efficient, statically typed programming language. "
    "Answer with one complete canonical Tokit program in a ```tokit code block."
)
REPAIR_SYSTEM = (
    "You fix Tokit programs. Answer with the complete corrected canonical Tokit "
    "program in a ```tokit code block."
)


# ---------------------------------------------------------------- rendering
def show_str(text: str) -> str:
    escaped = text.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\t", "\\t")
    return f'"{escaped}"'


def show(value) -> str:
    """Render a Python value the way Tokit prints a `main` result."""
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return show_str(value)
    if isinstance(value, list):
        return "[" + ",".join(show(item) for item in value) + "]"
    if isinstance(value, dict):
        return "{" + ",".join(f"{show(k)}:{show(v)}" for k, v in sorted(value.items())) + "}"
    if value is None:
        return "None"
    if isinstance(value, tuple) and value[0] == "Some":
        return f"Some({show(value[1])})"
    raise TypeError(value)


def lit(values: list[int]) -> str:
    return "[" + ",".join(str(v) for v in values) + "]"


def slit(values: list[str]) -> str:
    return "[" + ",".join(show_str(v) for v in values) + "]"


WORDS = ["apple", "river", "stone", "cloud", "tiger", "maple", "ember", "delta", "lotus", "pixel",
         "orbit", "cedar", "frost", "amber", "coral", "flint", "harbor", "ivory", "jade", "kiwi"]
NAMES = ["ada", "bo", "cy", "dee", "eli", "fay", "gus", "hal", "ivy", "jo", "kai", "lu"]


def pick(rng: random.Random, options):
    return options[rng.randrange(len(options))]


def ints(rng: random.Random, n: int, lo: int = -20, hi: int = 50) -> list[int]:
    return [rng.randint(lo, hi) for _ in range(n)]


# ---------------------------------------------------------------- families
# Each family returns (instruction, code, expected_output, extra) where extra
# may contain "args" and "stdin".

def fam_sum_filtered(rng):
    xs = ints(rng, rng.randint(4, 9))
    kind = pick(rng, ["even", "odd", "positive", "greater"])
    k = rng.randint(0, 20)
    pred = {"even": ("x%2==0", lambda x: x % 2 == 0, "even"),
            "odd": ("x%2!=0", lambda x: x % 2 != 0, "odd"),
            "positive": ("x>0", lambda x: x > 0, "positive"),
            "greater": (f"x>{k}", lambda x: x > k, f"greater than {k}")}[kind]
    fname = pick(rng, ["sum_matching", "total", "add_up", "sum_selected"])
    style = rng.randint(0, 2)
    if style == 0:
        body = f"{fname}(xs:[I])->I{{var t=0;for x in xs{{if {pred[0]}{{t=t+x;}}}}t}}"
    elif style == 1:
        body = f"{fname}(xs:[I])->I{{fold(filter(xs,|x|{pred[0]}),0,|t,x|t+x)}}"
    else:
        body = f"{fname}(xs:[I])->I{{var t=0;for i in range(0,len(xs)){{let x=xs[i];if {pred[0]}{{t=t+x;}}}}t}}"
    code = f"{body}\nmain()->I{{{fname}({lit(xs)})}}"
    instr = pick(rng, [
        f"Write a Tokit function `{fname}` that returns the sum of the {pred[2]} numbers in an `[I]`, and a `main` that applies it to {lit(xs)}.",
        f"In Tokit, sum only the {pred[2]} elements of the array {lit(xs)}. Put the logic in a function named `{fname}`.",
        f"Create a Tokit program whose main returns the total of all {pred[2]} values in {lit(xs)} using a helper `{fname}`.",
    ])
    return instr, code, show(sum(x for x in xs if pred[1](x))), {}


def fam_extreme(rng):
    xs = ints(rng, rng.randint(3, 8), -100, 100)
    want_max = rng.random() < 0.5
    word = "largest" if want_max else "smallest"
    fname = pick(rng, ["largest", "peak", "top"]) if want_max else pick(rng, ["smallest", "lowest", "bottom"])
    op = ">" if want_max else "<"
    style = rng.randint(0, 1)
    if style == 0:
        body = f"{fname}(xs:[I])->I{{var best=xs[0];for x in xs{{if x{op}best{{best=x;}}}}best}}"
    else:
        f = "max" if want_max else "min"
        body = f"{fname}(xs:[I])->I{{fold(xs,xs[0],|a,b|{f}(a,b))}}"
    code = f"{body}\nmain()->I{{{fname}({lit(xs)})}}"
    instr = pick(rng, [
        f"Write a Tokit function `{fname}` returning the {word} element of a non-empty `[I]`; main should return it for {lit(xs)}.",
        f"Return the {word} number in {lit(xs)} from a Tokit main, using a function called `{fname}`.",
    ])
    return instr, code, show(max(xs) if want_max else min(xs)), {}


def fam_count_prefix(rng):
    words = [pick(rng, WORDS) for _ in range(rng.randint(4, 8))]
    letter = pick(rng, sorted({w[0] for w in words}))
    mode = pick(rng, ["prefix", "contains"])
    sub = letter if mode == "prefix" else pick(rng, ["a", "e", "r", "o"])
    fn = "starts_with" if mode == "prefix" else "contains"
    fname = pick(rng, ["count_words", "how_many", "matches"])
    code = (f"{fname}(ws:[String],s:String)->I{{len(filter(ws,|w|{fn}(w,s)))}}\n"
            f"main()->I{{{fname}({slit(words)},{show_str(sub)})}}")
    desc = f"start with {show_str(sub)}" if mode == "prefix" else f"contain {show_str(sub)}"
    instr = pick(rng, [
        f"Count how many of the words {slit(words)} {desc} in Tokit, using a function `{fname}(ws,s)`.",
        f"Write Tokit code with a function `{fname}` that counts the strings in an array that {desc}; main checks {slit(words)}.",
    ])
    count = sum(1 for w in words if (w.startswith(sub) if mode == "prefix" else sub in w))
    return instr, code, show(count), {}


def fam_word_count(rng):
    words = [pick(rng, WORDS[:6]) for _ in range(rng.randint(5, 10))]
    text = " ".join(words)
    fname = pick(rng, ["word_counts", "tally", "frequencies"])
    code = (f"{fname}(text:String)->Map<String,I>{{var m:Map<String,I>=Map();for w in split(text,\" \"){{m[w]=get_or(m,w,0)+1;}}m}}\n"
            f"main()->Map<String,I>{{{fname}({show_str(text)})}}")
    instr = pick(rng, [
        f"Write a Tokit function `{fname}` that counts how often each space-separated word occurs and returns a `Map<String,I>`. Apply it to {show_str(text)}.",
        f"Using a Tokit Map, tally the words in {show_str(text)} with a function named `{fname}`.",
    ])
    return instr, code, show(dict(Counter(words))), {}


def fam_reverse_words(rng):
    words = [pick(rng, WORDS) for _ in range(rng.randint(3, 6))]
    sep = pick(rng, [" ", ",", "-"])
    text = sep.join(words)
    fname = pick(rng, ["reverse_words", "flip", "backwards"])
    code = (f"{fname}(s:String,sep:String)->String{{join(reverse(split(s,sep)),sep)}}\n"
            f"main()->String{{{fname}({show_str(text)},{show_str(sep)})}}")
    instr = pick(rng, [
        f"Reverse the order of the {show_str(sep)}-separated parts of {show_str(text)} in Tokit with a function `{fname}`.",
        f"Write a Tokit function `{fname}(s,sep)` that reverses the order of separated items; main applies it to {show_str(text)}.",
    ])
    return instr, code, show(sep.join(reversed(words))), {}


def fam_fizzbuzz(rng):
    a, b = rng.choice([(3, 5), (2, 7), (4, 6), (3, 7)])
    wa, wb = pick(rng, ["Fizz", "Ping", "Foo", "Tick"]), pick(rng, ["Buzz", "Pong", "Bar", "Tock"])
    n = rng.randint(8, 16)
    fname = pick(rng, ["label", "say", "word_for"])
    code = (f"{fname}(i:I)->String{{if i%{a*b}==0{{\"{wa}{wb}\"}}else if i%{a}==0{{\"{wa}\"}}else if i%{b}==0{{\"{wb}\"}}else{{String(i)}}}}\n"
            f"main()->[String]{{map(range(1,{n+1}),|i|{fname}(i))}}")
    out = []
    for i in range(1, n + 1):
        out.append(wa + wb if i % (a * b) == 0 else wa if i % a == 0 else wb if i % b == 0 else str(i))
    instr = (f"In Tokit, map the numbers 1 to {n} to strings: multiples of {a*b} become \"{wa}{wb}\", "
             f"multiples of {a} \"{wa}\", multiples of {b} \"{wb}\", others their decimal text. Use a function `{fname}`.")
    return instr, code, show(out), {}


def fam_factorial(rng):
    n = rng.randint(5, 20)
    style = rng.randint(0, 1)
    fname = pick(rng, ["factorial", "fact"])
    if style == 0:
        body = f"{fname}(n:I)->L{{var r=1i64;for i in range(2,n+1){{r=r*i64(i);}}r}}"
    else:
        body = f"{fname}(n:I)->L{{if n<2{{1i64}}else{{i64(n)*{fname}(n-1)}}}}"
    code = f"{body}\nmain()->L{{{fname}({n})}}"
    r = 1
    for i in range(2, n + 1):
        r *= i
    instr = pick(rng, [
        f"Compute {n}! in Tokit using `i64` arithmetic in a function `{fname}`.",
        f"Write a Tokit function `{fname}(n:I)->L` and return the factorial of {n} from main.",
    ])
    return instr, code, show(r), {}


def fam_fibonacci(rng):
    n = rng.randint(5, 40)
    fname = pick(rng, ["fib", "fibonacci", "nth_fib"])
    code = (f"{fname}(n:I)->L{{var a=0i64;var b=1i64;for i in range(0,n){{let next=a+b;a=b;b=next;}}a}}\n"
            f"main()->L{{{fname}({n})}}")
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    instr = f"Return the {n}th Fibonacci number (fib(0)=0, fib(1)=1) from a Tokit main, iteratively, in a function `{fname}`."
    return instr, code, show(a), {}


def fam_gcd(rng):
    x, y = rng.randint(10, 500), rng.randint(10, 500)
    want = pick(rng, ["gcd", "lcm"])
    if want == "gcd":
        code = f"gcd(a:I,b:I)->I{{if b==0{{a}}else{{gcd(b,a%b)}}}}\nmain()->I{{gcd({x},{y})}}"
        import math
        expected = math.gcd(x, y)
        instr = f"Write a recursive Tokit function `gcd` and return gcd({x},{y}) from main."
    else:
        code = (f"gcd(a:I,b:I)->I{{if b==0{{a}}else{{gcd(b,a%b)}}}}\n"
                f"lcm(a:I,b:I)->I{{a/gcd(a,b)*b}}\nmain()->I{{lcm({x},{y})}}")
        import math
        expected = x * y // math.gcd(x, y)
        instr = f"Compute the least common multiple of {x} and {y} in Tokit with helper functions `gcd` and `lcm`."
    return instr, code, show(expected), {}


def fam_primes(rng):
    n = rng.randint(10, 80)
    fname = pick(rng, ["is_prime", "prime"])
    code = (f"{fname}(n:I)->bool{{if n<2{{return false;}}var d=2;while d*d<=n{{if n%d==0{{return false;}}d=d+1;}}true}}\n"
            f"main()->[I]{{filter(range(0,{n+1}),|x|{fname}(x))}}")
    primes = [p for p in range(n + 1) if p >= 2 and all(p % d for d in range(2, int(p ** 0.5) + 1))]
    instr = pick(rng, [
        f"List all primes from 0 to {n} in Tokit using a trial-division function `{fname}`.",
        f"Write a Tokit program returning the prime numbers up to {n}; test primality in `{fname}`.",
    ])
    return instr, code, show(primes), {}


def fam_records_total(rng):
    items = [(pick(rng, WORDS), rng.randint(1, 50), rng.randint(1, 9)) for _ in range(rng.randint(2, 5))]
    sname = pick(rng, ["Item", "Line", "Entry"])
    code = (f"struct {sname}{{name:String,price:I,qty:I}}\n"
            f"total(items:[{sname}])->I{{fold(items,0,|t,i|t+i.price*i.qty)}}\n"
            f"main()->I{{total([{','.join(f'{sname}({show_str(n)},{p},{q})' for n, p, q in items)}])}}")
    instr = (f"Define a Tokit record `{sname}` with `name`, `price`, and `qty`, and a function `total` summing price*qty; "
             f"main returns the total for {', '.join(f'{n} ({p} x {q})' for n, p, q in items)}.")
    return instr, code, show(sum(p * q for _, p, q in items)), {}


def fam_enum_machine(rng):
    ops = [pick(rng, ["Add", "Sub", "Double", "Reset"]) for _ in range(rng.randint(3, 7))]
    args = [rng.randint(1, 9) for _ in ops]
    value = 0
    rendered = []
    for op, a in zip(ops, args):
        if op == "Add":
            value += a
            rendered.append(f"Cmd::Add({a})")
        elif op == "Sub":
            value -= a
            rendered.append(f"Cmd::Sub({a})")
        elif op == "Double":
            value *= 2
            rendered.append("Cmd::Double")
        else:
            value = 0
            rendered.append("Cmd::Reset")
    code = ("enum Cmd{Add(I),Sub(I),Double,Reset}\n"
            "apply(v:I,c:Cmd)->I{match c{Cmd::Add(n)=>v+n,Cmd::Sub(n)=>v-n,Cmd::Double=>v*2,Cmd::Reset=>0}}\n"
            f"main()->I{{fold([{','.join(rendered)}],0,|v,c|apply(v,c))}}")
    instr = ("Model calculator commands as a Tokit enum `Cmd{Add(I),Sub(I),Double,Reset}`, write `apply(v,c)` with an "
             f"exhaustive match, and fold the commands {', '.join(rendered)} starting from 0.")
    return instr, code, show(value), {}


def fam_option_find(rng):
    xs = ints(rng, rng.randint(3, 8), 0, 60)
    d = rng.randint(3, 9)
    fname = pick(rng, ["first_multiple", "find_first", "earliest"])
    code = (f"{fname}(xs:[I],d:I)->Option<I>{{for x in xs{{if x%d==0{{return Some(x);}}}}None}}\n"
            f"main()->Option<I>{{{fname}({lit(xs)},{d})}}")
    found = next((x for x in xs if x % d == 0), None)
    instr = f"Return the first multiple of {d} in {lit(xs)} as an `Option<I>` from a Tokit function `{fname}`, or `None`."
    return instr, code, show(("Some", found) if found is not None else None), {}


def fam_parse_sum(rng):
    good = [str(rng.randint(-50, 99)) for _ in range(rng.randint(2, 5))]
    bad = rng.random() < 0.4
    values = good + ([pick(rng, ["x1", "", "4.5", "--3"])] if bad else [])
    rng.shuffle(values)
    code = ("sum_all(xs:[String])->Result<I,ParseError>{var t=0;for s in xs{t=t+parse_i32(s)?;}Ok(t)}\n"
            f"main()->String{{match sum_all({slit(values)}){{Ok(n)=>String(n),Err(e)=>\"invalid input\"}}}}")
    expected = "invalid input" if bad else str(sum(int(v) for v in values))
    instr = (f"Parse every string in {slit(values)} with `parse_i32` and add them in a Tokit function `sum_all` that "
             "propagates errors with `?`; main returns the sum as text or \"invalid input\".")
    return instr, code, show(expected), {}


def fam_string_transform(rng):
    word = pick(rng, WORDS) + " " + pick(rng, WORDS)
    pad = " " * rng.randint(0, 3)
    a, b = pick(rng, ["a", "e", "o", "r"]), pick(rng, ["_", "*", "4", "3"])
    steps = pick(rng, ["upper", "replace", "both"])
    if steps == "upper":
        code = f"main()->String{{upper(trim({show_str(pad + word + pad)}))}}"
        expected = word.upper()
        instr = f"Trim and uppercase the string {show_str(pad + word + pad)} in Tokit."
    elif steps == "replace":
        code = f"main()->String{{replace(trim({show_str(pad + word + pad)}),{show_str(a)},{show_str(b)})}}"
        expected = word.replace(a, b)
        instr = f"In Tokit, trim {show_str(pad + word + pad)} and replace every {show_str(a)} with {show_str(b)}."
    else:
        code = f"main()->String{{upper(replace({show_str(word)},{show_str(a)},{show_str(b)}))}}"
        expected = word.replace(a, b).upper()
        instr = f"Replace each {show_str(a)} in {show_str(word)} by {show_str(b)}, then uppercase the result, in Tokit."
    return instr, code, show(expected), {}


def fam_palindrome(rng):
    base = pick(rng, ["level", "rotor", "civic", "kayak", "stone", "river", "noon", "refer", "tiger"])
    text = base if rng.random() < 0.5 else base + pick(rng, ["", "x"])
    fname = pick(rng, ["is_palindrome", "palindrome", "reads_same"])
    code = (f"{fname}(s:String)->bool{{let c=chars(s);var i=0;var j=len(c)-1;while i<j{{if c[i]!=c[j]{{return false;}}i=i+1;j=j-1;}}true}}\n"
            f"main()->bool{{{fname}({show_str(text)})}}")
    instr = f"Write a Tokit function `{fname}` that checks with two indices whether a string reads the same backwards; test {show_str(text)}."
    return instr, code, show(text == text[::-1]), {}


def fam_matrix(rng):
    n = rng.randint(2, 4)
    m = [[rng.randint(0, 9) for _ in range(n)] for _ in range(n)]
    want = pick(rng, ["transpose", "diagonal"])
    rows = "[" + ",".join(lit(r) for r in m) + "]"
    if want == "transpose":
        code = (f"transpose(m:[[I]])->[[I]]{{var t=m;for i in range(0,len(m)){{for j in range(0,len(m)){{t[j][i]=m[i][j];}}}}t}}\n"
                f"main()->[[I]]{{transpose({rows})}}")
        expected = [[m[i][j] for i in range(n)] for j in range(n)]
        instr = f"Transpose the square matrix {rows} in Tokit by copying it and assigning `t[j][i]=m[i][j]`."
    else:
        code = (f"trace(m:[[I]])->I{{var t=0;for i in range(0,len(m)){{t=t+m[i][i];}}t}}\n"
                f"main()->I{{trace({rows})}}")
        expected = sum(m[i][i] for i in range(n))
        instr = f"Return the sum of the main diagonal of {rows} from a Tokit function `trace`."
    return instr, code, show(expected), {}


def fam_sort_records(rng):
    people = [(n, rng.randint(18, 80)) for n in rng.sample(NAMES, rng.randint(3, 6))]
    desc = rng.random() < 0.5
    key = "0-p.age" if desc else "p.age"
    order = "oldest first" if desc else "youngest first"
    code = (f"struct Person{{name:String,age:I}}\n"
            f"main()->[String]{{map(sort_by([{','.join(f'Person({show_str(n)},{a})' for n, a in people)}],|p|{key}),|p|p.name)}}")
    ordered = sorted(people, key=lambda p: -p[1] if desc else p[1])
    instr = (f"Sort these people {', '.join(f'{n} ({a})' for n, a in people)} {order} with Tokit's `sort_by` "
             "and return their names.")
    return instr, code, show([n for n, _ in ordered]), {}


def fam_closures(rng):
    k = rng.randint(2, 9)
    c = rng.randint(1, 20)
    xs = ints(rng, rng.randint(2, 5), 0, 20)
    code = (f"multiplier(k:I)->(I)->I{{|x:I|x*k}}\n"
            f"compose(f:(I)->I,g:(I)->I)->(I)->I{{|x:I|g(f(x))}}\n"
            f"main()->[I]{{let h=compose(multiplier({k}),|x|x+{c});map({lit(xs)},h)}}")
    instr = (f"In Tokit, write `multiplier(k)` returning a lambda and `compose(f,g)`; apply `x*{k}` then `+{c}` "
             f"to every element of {lit(xs)} with `map`.")
    return instr, code, show([x * k + c for x in xs]), {}


def fam_generic(rng):
    vals = [pick(rng, WORDS) for _ in range(rng.randint(0, 3))]
    default = pick(rng, ["none", "empty", "?"])
    code = (f"first_or<T>(xs:[T],d:T)->T{{if len(xs)>0{{xs[0]}}else{{d}}}}\n"
            f"main()->String{{let xs:[String]={slit(vals)};first_or(xs,{show_str(default)})}}")
    instr = f"Write a generic Tokit function `first_or<T>(xs:[T],d:T)->T` and use it on {slit(vals)} with default {show_str(default)}."
    return instr, code, show(vals[0] if vals else default), {}


def fam_binary_search(rng):
    xs = sorted(rng.sample(range(0, 100), rng.randint(4, 12)))
    target = pick(rng, xs) if rng.random() < 0.6 else rng.randint(0, 100)
    code = ("search(xs:[I],t:I)->Option<I>{var lo=0;var hi=len(xs);while lo<hi{let mid=(lo+hi)/2;if xs[mid]==t{return Some(mid);}if xs[mid]<t{lo=mid+1;}else{hi=mid;}}None}\n"
            f"main()->Option<I>{{search({lit(xs)},{target})}}")
    expected = ("Some", xs.index(target)) if target in xs else None
    instr = f"Binary-search {target} in the sorted array {lit(xs)} in Tokit; return its index as `Some(i)` or `None`."
    return instr, code, show(expected), {}


def fam_bubble_sort(rng):
    xs = ints(rng, rng.randint(3, 8))
    code = ("bubble(input:[I])->[I]{var xs=input;let n=len(xs);for i in range(0,n){for j in range(0,n-1-i){if xs[j]>xs[j+1]{let t=xs[j];xs[j]=xs[j+1];xs[j+1]=t;}}}xs}\n"
            f"main()->[I]{{bubble({lit(xs)})}}")
    instr = f"Implement bubble sort in Tokit using element assignment and sort {lit(xs)}."
    return instr, code, show(sorted(xs)), {}


def fam_char_freq(rng):
    word = pick(rng, WORDS) + pick(rng, WORDS)
    code = ("letters(s:String)->Map<String,I>{var m:Map<String,I>=Map();for c in chars(s){m[c]=get_or(m,c,0)+1;}m}\n"
            f"main()->Map<String,I>{{letters({show_str(word)})}}")
    instr = f"Count each character of {show_str(word)} in a Tokit `Map<String,I>` using `chars`."
    return instr, code, show(dict(Counter(word))), {}


def fam_temperature(rng):
    cs = [rng.randint(-30, 45) for _ in range(rng.randint(2, 5))]
    code = (f"to_f(c:I)->Option<I>{{i32(round(f64(c)*9.0/5.0+32.0))}}\n"
            f"main()->[Option<I>]{{map({lit(cs)},|c|to_f(c))}}")

    def rnd(x):
        return int(x + 0.5) if x >= 0 else -int(-x + 0.5)
    expected = [("Some", rnd(c * 9 / 5 + 32)) for c in cs]
    instr = (f"Convert the Celsius values {lit(cs)} to Fahrenheit in Tokit using `f64`, `round`, and `i32(...)`, "
             "returning `[Option<I>]`.")
    return instr, code, show(expected), {}


def fam_brackets(rng):
    s = "".join(pick(rng, ["()", "[]", "{}", "(", ")", "[", "]"]) for _ in range(rng.randint(2, 6)))
    code = ("balanced(s:String)->bool{var stack:[String]=[];for c in chars(s){match c{\"(\"=>{stack.push(\")\");},\"[\"=>{stack.push(\"]\");},\"{\"=>{stack.push(\"}\");},_=>{if last(stack)!=Some(c){return false;}stack.pop();}}}len(stack)==0}\n"
            f"main()->bool{{balanced({show_str(s)})}}")
    pairs = {")": "(", "]": "[", "}": "{"}
    st = []
    ok = True
    for ch in s:
        if ch in "([{":
            st.append(ch)
        elif not st or st[-1] != pairs[ch]:
            ok = False
            break
        else:
            st.pop()
    ok = ok and not st
    instr = f"Check in Tokit whether the brackets in {show_str(s)} are balanced, using a stack and a string `match`."
    return instr, code, show(ok), {}


def fam_rle(rng):
    s = "".join(pick(rng, ["a", "b", "c"]) * rng.randint(1, 4) for _ in range(rng.randint(2, 5)))
    code = ("encode(s:String)->String{let cs=chars(s);var out=\"\";var i=0;while i<len(cs){var j=i;while j<len(cs)&&cs[j]==cs[i]{j=j+1;}out=out+String(j-i)+cs[i];i=j;}out}\n"
            f"main()->String{{encode({show_str(s)})}}")
    out, i = "", 0
    while i < len(s):
        j = i
        while j < len(s) and s[j] == s[i]:
            j += 1
        out += str(j - i) + s[i]
        i = j
    instr = f"Run-length encode {show_str(s)} in Tokit as count followed by character, e.g. \"aab\" -> \"2a1b\"."
    return instr, code, show(out), {}


def fam_cli_args(rng):
    values = [str(rng.randint(-20, 99)) for _ in range(rng.randint(1, 5))]
    code = ("main()->Unit{var t=0;for a in args(){match parse_i32(a){Ok(n)=>{t=t+n;},Err(e)=>{print(\"bad: \"+a);exit(1);}}}print(String(t));}")
    instr = "Write a Tokit CLI that prints the sum of all integer command-line arguments, or prints \"bad: <arg>\" and exits with status 1."
    return instr, code, str(sum(int(v) for v in values)) + "\n", {"args": values, "raw": True}


def fam_stdin_lines(rng):
    lines = [" ".join(pick(rng, WORDS) for _ in range(rng.randint(1, 4))) for _ in range(rng.randint(1, 5))]
    code = ("main()->Unit{let text=match read_stdin(){Ok(t)=>t,Err(e)=>\"\"};var words=0;for line in lines(text){words=words+len(filter(split(line,\" \"),|w|w!=\"\"));}print(String(len(lines(text)))+\" \"+String(words));}")
    instr = "Read all of standard input in Tokit and print the number of lines and words separated by a space."
    stdin = "\n".join(lines) + "\n"
    return instr, code, f"{len(lines)} {sum(len(l.split()) for l in lines)}\n", {"stdin": stdin, "raw": True}


def fam_tree(rng):
    depth = rng.randint(1, 4)

    def build(d):
        if d == 0:
            return "Tree::Leaf", 0
        kids = rng.randint(1, 2)
        parts = [build(d - 1) for _ in range(kids)]
        return f"Tree::Node([{','.join(p for p, _ in parts)}])", 1 + max(h for _, h in parts)
    tree, height = build(depth)
    code = ("enum Tree{Leaf,Node([Tree])}\n"
            "height(t:Tree)->I{match t{Tree::Leaf=>0,Tree::Node(cs)=>1+fold(cs,0,|m,c|max(m,height(c)))}}\n"
            f"main()->I{{height({tree})}}")
    instr = f"Define a Tokit enum `Tree{{Leaf,Node([Tree])}}` and a recursive `height`; return the height of {tree}."
    return instr, code, show(height), {}


def fam_dispatch(rng):
    cmds = [pick(rng, ["inc", "dec", "double", "noop", "zero"]) for _ in range(rng.randint(3, 7))]
    code = ("run(v:I,cmd:String)->I{match cmd{\"inc\"=>v+1,\"dec\"=>v-1,\"double\"=>v*2,\"zero\"=>0,_=>v}}\n"
            f"main()->I{{fold({slit(cmds)},1,|v,c|run(v,c))}}")
    v = 1
    for c in cmds:
        v = {"inc": v + 1, "dec": v - 1, "double": v * 2, "zero": 0}.get(c, v)
    instr = f"Interpret the commands {slit(cmds)} starting from 1 in Tokit with a string `match` (inc, dec, double, zero; others do nothing)."
    return instr, code, show(v), {}


def fam_http_handler(rng):
    route = pick(rng, ["/health", "/status", "/ping"])
    reply = pick(rng, ["ok", "alive", "pong"])
    code = (f"handle(r:Request)->Response{{match r.path{{{show_str(route)}=>Response(200,Map(),{show_str(reply)}),_=>Response(404,Map(),\"not found\")}}}}\n"
            "main()->Unit{match serve(\"127.0.0.1:8080\",0,|r|handle(r)){Ok(u)=>{},Err(e)=>{print(\"cannot serve\");exit(1);}}}")
    instr = (f"Write a Tokit HTTP server on 127.0.0.1:8080 that answers {route} with 200 \"{reply}\" and everything "
             "else with 404, printing \"cannot serve\" and exiting with 1 if serving fails.")
    return instr, code, None, {"check_only": True}


def fam_nested_patterns(rng):
    pool = ["0", "7", "-3", "42", "x1", "", "99999999999", "15", "-8", "abc"]
    items = [pick(rng, pool) for _ in range(rng.randint(3, 6))]
    fname = pick(rng, ["classify", "kind", "label"])
    code = (f"{fname}(s:String)->String{{match parse_i32(s){{Ok(0)=>\"zero\",Ok(n)=>if n<0{{\"negative\"}}else{{\"positive\"}},"
            "Err(ParseError::Invalid)=>\"invalid\",Err(e)=>\"out of range\"}}\n"
            f"main()->[String]{{map({slit(items)},|s|{fname}(s))}}")

    def classify(text):
        try:
            n = int(text)
        except ValueError:
            return "invalid"
        if not -2**31 <= n < 2**31:
            return "out of range"
        return "zero" if n == 0 else ("negative" if n < 0 else "positive")
    instr = (f"In Tokit, classify each of {slit(items)} with `{fname}`: parse it with `parse_i32` and use one nested "
             "`match` that answers \"zero\", \"negative\", \"positive\", \"invalid\" for `ParseError::Invalid`, "
             "and \"out of range\" for other errors.")
    return instr, code, show([classify(t) for t in items]), {}


def fam_structural_eq(rng):
    points = [(rng.randint(0, 3), rng.randint(0, 3)) for _ in range(rng.randint(4, 8))]
    target = pick(rng, points) if rng.random() < 0.8 else (9, 9)
    rendered = ",".join(f"P({x},{y})" for x, y in points)
    code = ("struct P{x:I,y:I}\n"
            f"main()->I{{let ps=[{rendered}];len(filter(ps,|p|p==P({target[0]},{target[1]})))}}")
    instr = (f"Count in Tokit how many points in [{rendered}] equal P({target[0]},{target[1]}), "
             "using a record `P{x:I,y:I}` and `==` on records.")
    return instr, code, show(sum(1 for p in points if p == target)), {}


def fam_option_eq(rng):
    xs = ints(rng, rng.randint(3, 7), 0, 30)
    v = pick(rng, xs) if rng.random() < 0.6 else 99
    code = ("index_of(xs:[I],v:I)->Option<I>{var i=0;while i<len(xs){if xs[i]==v{return Some(i);}i=i+1;}None}\n"
            f"main()->bool{{index_of({lit(xs)},{v})==None}}")
    instr = (f"Write `index_of(xs,v)->Option<I>` in Tokit and return whether {v} is missing from {lit(xs)} "
             "by comparing the result with `None`.")
    return instr, code, show(v not in xs), {}


def fam_map_inventory(rng):
    words = [pick(rng, WORDS[:6]) for _ in range(rng.randint(4, 9))]
    code = ("count(ws:[String])->Map<String,I>{var m:Map<String,I>=Map();for w in ws{m[w]=get_or(m,w,0)+1;}m}\n"
            f"main()->Map<String,I>{{count({slit(words)})}}")
    counts = {}
    for w in words:
        counts[w] = counts.get(w, 0) + 1
    instr = f"Count how often each word occurs in {slit(words)} with a Tokit `Map<String,I>` and return the map."
    return instr, code, show(counts), {}


def fam_digest(rng):
    import hashlib
    text = pick(rng, WORDS) + str(rng.randint(0, 99))
    algo = pick(rng, ["sha256", "sha1", "md5"])
    code = f"main()->String{{hex({algo}(utf8_encode({show_str(text)})))}}"
    instr = f"Return the lowercase hex {algo.upper()} digest of the text {show_str(text)} in Tokit."
    return instr, code, show(hashlib.new(algo, text.encode()).hexdigest()), {}


def fam_bits(rng):
    a, b = rng.randint(0, 255), rng.randint(0, 255)
    k = rng.randint(0, 4)
    code = f"main()->[I]{{let a={a};let b={b};[bit_and(a,b),bit_or(a,b),bit_xor(a,b),shl(a,{k}),shr(b,{k})]}}"
    instr = (f"In Tokit, return [a AND b, a OR b, a XOR b, a shifted left by {k}, b shifted right by {k}] "
             f"for a={a}, b={b} (Tokit has bit functions, not operators).")
    return instr, code, show([a & b, a | b, a ^ b, a << k, b >> k]), {}


def fam_crud_server(rng):
    prefix = pick(rng, ["items", "notes", "keys"])
    code = ("reply(s:I,b:String)->Response{let h:Map<String,String>=Map();Response(s,h,b)}\n"
            "main()->Unit{let l=match listen(\"127.0.0.1:8080\"){Ok(l)=>l,Err(e)=>{print(\"cannot listen\");exit(1)}};"
            "var store:Map<String,String>=Map();while true{let c=match accept(l){Ok(c)=>c,Err(e)=>{continue;}};"
            f"match http_read(c){{Ok(r)=>{{let parts=split(r.path,\"/\");if len(parts)==3&&parts[1]=={show_str(prefix)}{{"
            "let k=parts[2];match r.method{\"PUT\"=>{store[k]=r.body;http_write(c,reply(201,\"stored\"));},"
            "\"GET\"=>{http_write(c,match get(store,k){Some(v)=>reply(200,v),None=>reply(404,\"not found\")});},"
            "_=>{http_write(c,reply(405,\"method not allowed\"));}}}else{http_write(c,reply(404,\"not found\"));}},"
            "Err(e)=>{http_write(c,reply(400,\"bad request\"));}}tcp_close(c);}}")
    instr = (f"Write a Tokit server on 127.0.0.1:8080 that keeps an in-memory `Map<String,String>`: PUT /{prefix}/<key> "
             f"stores the body (201), GET /{prefix}/<key> returns it or 404, other methods get 405. Use "
             "`listen`, `accept`, `http_read`, `http_write`, and close each connection.")
    return instr, code, None, {"check_only": True}


def fam_worker_server(rng):
    workers = pick(rng, [4, 8, 16])
    code = ("handle(r:Request)->Response{let h:Map<String,String>=Map();match r.path{\"/work\"=>{sleep_ms(50i64);"
            "Response(200,h,\"done\")},_=>Response(404,h,\"not found\")}}\n"
            f"main()->Unit{{match serve(\"127.0.0.1:8080\",0,{workers},|r|handle(r)){{Ok(u)=>{{}},"
            "Err(e)=>{print(\"cannot serve\");exit(1);}}}")
    instr = (f"Write a concurrent Tokit HTTP service on 127.0.0.1:8080 with {workers} workers: /work sleeps 50 ms and "
             "answers \"done\", anything else is 404.")
    return instr, code, None, {"check_only": True}


def fam_rpn(rng):
    tokens, depth, values = [], 0, []
    for _ in range(rng.randint(3, 6)):
        if depth >= 2 and rng.random() < 0.5:
            op = pick(rng, ["+", "-", "*"])
            tokens.append(op)
            b, a = values.pop(), values.pop()
            values.append(a + b if op == "+" else a - b if op == "-" else a * b)
            depth -= 1
        else:
            n = rng.randint(1, 9)
            tokens.append(str(n))
            values.append(n)
            depth += 1
    while depth >= 2:
        op = pick(rng, ["+", "*"])
        tokens.append(op)
        b, a = values.pop(), values.pop()
        values.append(a + b if op == "+" else a * b)
        depth -= 1
    code = ("eval(ts:[String])->Option<I>{var st:[I]=[];for t in ts{match t{\"+\"=>{let b=last(st)?;st.pop();let a=last(st)?;st.pop();st.push(a+b);},"
            "\"-\"=>{let b=last(st)?;st.pop();let a=last(st)?;st.pop();st.push(a-b);},\"*\"=>{let b=last(st)?;st.pop();let a=last(st)?;st.pop();st.push(a*b);},"
            "_=>{st.push(parse_i32(t).ok()?);}}}last(st)}\n"
            f"main()->Option<I>{{eval({slit(tokens)})}}")
    code = code.replace("parse_i32(t).ok()?", "match parse_i32(t){Ok(n)=>n,Err(e)=>{return None;}}")
    instr = (f"Evaluate the reverse Polish expression {slit(tokens)} in Tokit with a stack: push numbers, and for "
             "an operator take the top two with `last` and `pop`. Return the result as `Option<I>`.")
    return instr, code, show(("Some", values[-1])), {}


def fam_undo(rng):
    ops = []
    for _ in range(rng.randint(4, 8)):
        ops.append("undo" if ops and rng.random() < 0.35 else pick(rng, WORDS[:8]))
    code = ("apply(ops:[String])->[String]{var doc:[String]=[];for o in ops{if o==\"undo\"{doc.pop();}else{doc.push(o);}}doc}\n"
            f"main()->[String]{{apply({slit(ops)})}}")
    doc = []
    for o in ops:
        if o == "undo":
            if doc:
                doc.pop()
        else:
            doc.append(o)
    instr = (f"Replay the edits {slit(ops)} in Tokit: each word is appended to a document, and \"undo\" removes "
             "the most recent word (if any) with `pop`. Return the final document.")
    return instr, code, show(doc), {}


def fam_dedupe(rng):
    xs = []
    for _ in range(rng.randint(5, 10)):
        xs.append(xs[-1] if xs and rng.random() < 0.4 else rng.randint(0, 5))
    code = ("dedupe(xs:[I])->[I]{var out:[I]=[];for x in xs{if last(out)!=Some(x){out.push(x);}}out}\n"
            f"main()->[I]{{dedupe({lit(xs)})}}")
    out = []
    for x in xs:
        if not out or out[-1] != x:
            out.append(x)
    instr = f"Remove adjacent duplicates from {lit(xs)} in Tokit, comparing each value with `last` of the result."
    return instr, code, show(out), {}


def fam_expr_tree(rng):
    def build(d):
        if d == 0 or rng.random() < 0.3:
            n = rng.randint(1, 6)
            return f"Expr::Num({n})", n
        kind = pick(rng, ["Add", "Mul"])
        parts = [build(d - 1) for _ in range(rng.randint(2, 3))]
        value = sum(v for _, v in parts) if kind == "Add" else 1
        if kind == "Mul":
            for _, v in parts:
                value *= v
        return f"Expr::{kind}([{','.join(t for t, _ in parts)}])", value
    tree, value = build(rng.randint(1, 3))
    code = ("enum Expr{Num(I),Add([Expr]),Mul([Expr])}\n"
            "eval(e:Expr)->I{match e{Expr::Num(n)=>n,Expr::Add(xs)=>fold(xs,0,|a,x|a+eval(x)),Expr::Mul(xs)=>fold(xs,1,|a,x|a*eval(x))}}\n"
            f"main()->I{{eval({tree})}}")
    instr = (f"Define a Tokit enum `Expr{{Num(I),Add([Expr]),Mul([Expr])}}` and a recursive `eval` with `fold`; "
             f"evaluate {tree}.")
    return instr, code, show(value), {}


FAMILIES: dict[str, Callable] = {name[4:]: fn for name, fn in globals().items() if name.startswith("fam_")}
# Held out entirely for the out-of-distribution test split.
OOD_FAMILIES = ["rle", "brackets", "binary_search", "tree", "dispatch"]


# ---------------------------------------------------------------- tooling
class Tok:
    def __init__(self, path: Path, work: Path):
        self.path = path
        self.work = work
        self.counter = 0

    def file(self, code: str) -> Path:
        self.counter += 1
        path = self.work / f"s{self.counter}.tok"
        path.write_text(code, encoding="utf-8", newline="\n")
        return path

    def run(self, *args: str, stdin: str | None = None) -> subprocess.CompletedProcess:
        return subprocess.run([str(self.path), *args], input=stdin, capture_output=True, text=True,
                              encoding="utf-8", timeout=60)

    def canonical(self, code: str) -> str | None:
        path = self.file(code)
        if self.run("compact", "--write", str(path)).returncode != 0:
            return None
        if self.run("fmt", "--write", str(path)).returncode != 0:
            return None
        return path.read_text(encoding="utf-8").strip()

    def diagnostic(self, code: str) -> dict | None:
        result = self.run("check", "--json", str(self.file(code)))
        data = json.loads(result.stdout or "{}")
        return None if data.get("ok") else data.get("error")

    def output(self, code: str, args: list[str], stdin: str | None) -> tuple[int, str]:
        result = self.run("run", str(self.file(code)), *(["--", *args] if args else []), stdin=stdin)
        return result.returncode, result.stdout.replace("\r\n", "\n")


# ---------------------------------------------------------------- repairs
def mutations(code: str, rng: random.Random) -> list[tuple[str, str]]:
    """Plausible model mistakes; only those producing a diagnostic are kept."""
    import re
    out = []
    positions = [i for i, ch in enumerate(code) if ch == ";"]
    if positions:
        at = pick(rng, positions)
        out.append(("missing semicolon", code[:at] + code[at + 1:]))
    if "->I{" in code:
        out.append(("wrong return type", code.replace("->I{", "->String{", 1)))
    if match := re.search(r"let (\w+)=", code):
        name = match.group(1)
        rest = code[match.end():]
        if re.search(rf"\b{name}\b", rest):
            out.append(("misspelled name", code[:match.end()] + re.sub(rf"\b{name}\b", name + "s", rest, count=1)))
    if "var " in code:
        out.append(("assigned immutable binding", code.replace("var ", "let ", 1)))
    if match := re.search(r"\blen\((\w+)\)", code):
        out.append(("method call syntax", code[:match.start()] + f"{match.group(1)}.len()" + code[match.end():]))
    if "&&" in code:
        out.append(("Python boolean operator", code.replace("&&", " and ", 1)))
    if match := re.search(r"String\((\w+)\)", code):
        out.append(("to_string method", code[:match.start()] + f"{match.group(1)}.to_string()" + code[match.end():]))
    if match := re.search(r"\b[A-Z]\w*::(\w+)", code):
        out.append(("unqualified variant", code[:match.start()] + match.group(1) + code[match.end():]))
    if "=>" in code:
        out.append(("arrow instead of fat arrow", code.replace("=>", "->", 1)))
    if "var " in code:
        out.append(("Rust let mut", code.replace("var ", "let mut ", 1)))
    if ":[String]" in code:
        out.append(("Rust char type", code.replace(":[String]", ":[char]", 1)))
    if match := re.search(r"\blast\((\w+)\)", code):
        out.append(("method last", code[:match.start()] + f"{match.group(1)}.last()" + code[match.end():]))
    if match := re.search(r"(\w+)\.pop\(\);", code):
        out.append(("pop as expression", code[:match.start()] + f"let top={match.group(1)}.pop();" + code[match.end():]))
    rng.shuffle(out)
    return out


# ---------------------------------------------------------------- main
def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tok", type=Path, required=True, help="tok executable (release build recommended)")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--per-family", type=int, default=40)
    parser.add_argument("--repairs-per-family", type=int, default=15)
    parser.add_argument("--seed", type=int, default=20261006)
    args = parser.parse_args()
    rng = random.Random(args.seed)
    args.out.mkdir(parents=True, exist_ok=True)
    rejected = Counter()
    samples = []
    with tempfile.TemporaryDirectory(prefix="tokit-dataset-") as work:
        tok = Tok(args.tok.resolve(), Path(work))
        for family, generate in FAMILIES.items():
            seen = set()
            verified = []
            attempts = 0
            while len(verified) < args.per_family and attempts < args.per_family * 6:
                attempts += 1
                instruction, code, expected, extra = generate(rng)
                canonical = tok.canonical(code)
                if canonical is None:
                    rejected[f"{family}:unparsable"] += 1
                    continue
                if (instruction, canonical) in seen:
                    continue
                if (error := tok.diagnostic(canonical)) is not None:
                    rejected[f"{family}:check {error.get('code')}"] += 1
                    continue
                if not extra.get("check_only"):
                    status, stdout = tok.output(canonical, extra.get("args", []), extra.get("stdin"))
                    printed = stdout if extra.get("raw") else stdout.strip()
                    if status != 0 or printed != expected:
                        rejected[f"{family}:output"] += 1
                        continue
                seen.add((instruction, canonical))
                verified.append(canonical)
                samples.append({
                    "messages": [
                        {"role": "system", "content": SYSTEM},
                        {"role": "user", "content": instruction},
                        {"role": "assistant", "content": f"```tokit\n{canonical}\n```"},
                    ],
                    "meta": {"family": family, "kind": "generate", "expected": expected,
                             "args": extra.get("args", []), "stdin": extra.get("stdin"),
                             "check_only": bool(extra.get("check_only"))},
                })
            repairs = 0
            for canonical in verified:
                if repairs >= args.repairs_per_family:
                    break
                for label, broken in mutations(canonical, rng):
                    error = tok.diagnostic(broken)
                    if error is None or repairs >= args.repairs_per_family:
                        continue
                    repairs += 1
                    diagnostic = f"{error['code']} at {error['line']}:{error['column']}: {error['message']}"
                    samples.append({
                        "messages": [
                            {"role": "system", "content": REPAIR_SYSTEM},
                            {"role": "user", "content": f"This Tokit program fails `tok check` with `{diagnostic}`.\n\n```tokit\n{broken}\n```"},
                            {"role": "assistant", "content": f"```tokit\n{canonical}\n```"},
                        ],
                        "meta": {"family": family, "kind": "repair", "mutation": label, "code": error["code"]},
                    })
                    break
            print(f"{family:16s} verified={len(verified):3d} repairs={repairs:3d}")

    ood = [s for s in samples if s["meta"]["family"] in OOD_FAMILIES]
    rest = [s for s in samples if s["meta"]["family"] not in OOD_FAMILIES]
    rng.shuffle(rest)
    n = len(rest)
    splits = {"train": rest[: int(n * 0.9)], "val": rest[int(n * 0.9): int(n * 0.95)],
              "test_iid": rest[int(n * 0.95):], "test_ood": ood}
    for name, rows in splits.items():
        with (args.out / f"{name}.jsonl").open("w", encoding="utf-8", newline="\n") as handle:
            for row in rows:
                handle.write(json.dumps(row, ensure_ascii=False) + "\n")
    stats = {
        "seed": args.seed,
        "tok_sha256": hashlib.sha256(args.tok.read_bytes()).hexdigest(),
        "families": sorted(FAMILIES),
        "ood_families": OOD_FAMILIES,
        "splits": {name: {"rows": len(rows), "kinds": Counter(r["meta"]["kind"] for r in rows)}
                   for name, rows in splits.items()},
        "rejected": dict(rejected),
    }
    (args.out / "stats.json").write_text(json.dumps(stats, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(stats["splits"], indent=2))
    if rejected:
        print("rejected:", dict(rejected))


if __name__ == "__main__":
    main()
