# Grammar

This is the grammar the bootstrap compiler accepts
([lexer](../compiler/src/lexer.rs), [parser](../compiler/src/parser.rs)).
Where this document and the implementation disagree, the implementation is
authoritative and the document is a bug. Semantics live in
[EXPERIMENTAL_SUBSET.md](EXPERIMENTAL_SUBSET.md) and
[TYPE_SYSTEM.md](TYPE_SYSTEM.md).

## Lexical grammar

Source files are UTF-8. Whitespace (ASCII space, tab, carriage return, line
feed) separates tokens and is otherwise insignificant. `//` starts a comment
that runs to the end of the line; there are no block comments.

```ebnf
ident      = ( letter | "_" ) { letter | digit | "_" } ;      (* ASCII only *)
letter     = "a".."z" | "A".."Z" ;
digit      = "0".."9" ;
int        = digit { digit } ;                                  (* i32 *)
int64      = digit { digit } "i64" ;                            (* i64; "i64" not followed by a name character *)
float      = digit { digit } ( fraction [ exponent ] | exponent ) ;
fraction   = "." digit { digit } ;
exponent   = ( "e" | "E" ) [ "+" | "-" ] digit { digit } ;
string     = '"' { char | escape } '"' ;                        (* no raw line breaks *)
escape     = "\" ( "n" | "t" | "r" | '"' | "\" ) ;
```

Keywords: `fn struct enum import pub let var for while break continue in if
match spawn else return true false Ok Err Some None`. `fn` is optional before
a function and canonical formatting removes it.

Punctuation, longest match first: `:: -> => == != <= >= && || ( ) { } [ ] :
, ; + - * / % = ! | < > ? .`

A minus sign directly followed by a literal (no space) forms a negative
literal. Any other character reports `E001`; an unclosed string or an unknown
escape reports `E004`; an out-of-range or malformed number reports `E003`.

## Syntactic grammar

```ebnf
program     = { import } { declaration } ;
import      = "import" ident "=" string ";" ;           (* "./x.tok" or "pkg:name" *)
declaration = [ "pub" ] ( record | enum | function ) ;

record      = "struct" ident [ generics ] "{" [ field { "," field } ] "}" ;
field       = ident ":" type ;
enum        = "enum" ident [ generics ] "{" [ variant { "," variant } ] "}" ;
variant     = ident [ "(" type ")" ] ;
function    = [ "fn" ] ident [ generics ] "(" [ param { "," param } ] ")" "->" type block ;
param       = ident ":" type ;
generics    = "<" ident { "," ident } ">" ;

type        = "[" type "]"                               (* array *)
            | "(" [ type { "," type } ] ")" "->" type    (* function value *)
            | ( "i32" | "I" | "i64" | "L" | "f64" | "F" | "bool" | "String" | "Bytes" | "Unit" )
            | ( "Result" | "Option" | "Task" | name ) [ "<" type { "," type } ">" ] ;
name        = ident [ "::" ident ] ;                     (* alias::Type for imports *)

block       = "{" { statement } [ expr ] "}" ;
statement   = ( "let" | "var" ) ident [ ":" type ] "=" expr ";"
            | "for" ident "in" expr block
            | "while" expr block
            | place "=" expr ";"
            | ident "." "push" "(" expr ")" ";"
            | ( "break" | "continue" ) ";"              (* only inside a loop body *)
            | "return" expr ";"
            | expr ";"
            | block_like ;                               (* if, match, or block without ";" *)
place       = ident { "." ident | "[" expr "]" } ;

expr        = unary { binop unary } ;                    (* precedence climbing, left associative *)
binop       = "||" | "&&" | "==" | "!=" | "<" | "<=" | ">" | ">="
            | "+" | "-" | "*" | "/" | "%" ;
unary       = ( "!" | "-" ) unary | postfix ;
postfix     = atom { "." ident | "[" expr "]" | call_args | "?" } ;
                                                         (* call_args only after ".", "[...]", or another call *)
atom        = literal | "(" ")" | "(" expr ")" | "[" [ expr { "," expr } ] "]"
            | ident [ call_args ]                        (* variable, function, or record constructor *)
            | name "::" ident [ call_args ]              (* enum variant (one payload) or imported function *)
            | ( "Ok" | "Err" | "Some" ) "(" expr ")" | "None"
            | lambda | block_like | "spawn" ident call_args ;
call_args   = "(" [ expr { "," expr } ] ")" ;
lambda      = "|" [ lparam { "," lparam } ] "|" expr | "||" expr ;
lparam      = ident [ ":" type ] ;
block_like  = block
            | "if" expr block [ "else" ( block | if_expr ) ]
            | "match" expr "{" [ arm { "," arm } ] "}" ;
arm         = pattern "=>" expr ;
literal     = int | int64 | float | string | "true" | "false" ;

pattern     = "_" | ident                                (* wildcard, binding *)
            | [ "-" ] int | [ "-" ] int64 | string | "true" | "false"
            | ( "Ok" | "Err" | "Some" ) "(" pattern ")" | "None"
            | name "::" ident [ "(" pattern ")" ] ;
```

Binary operator precedence, loosest first: `||`; `&&`; `==` `!=`; `<` `<=`
`>` `>=`; `+` `-`; `*` `/` `%`. Unary `!` and `-` bind tighter than every
binary operator and looser than postfix field access, indexing, calls, and
`?`. There are no assignment expressions, no compound assignment operators,
and no bitwise operators (`|` is reserved for lambdas; see `bit_and` and
friends).

## Disambiguation rules

- A `>` that closes type arguments may be glued to `=` (`Map<K,V>=Map()`);
  the parser splits `>=` in type position.
- An identifier followed by `=` at the start of a statement is an assignment;
  otherwise a statement starting with an expression becomes an assignment
  when `=` follows a place.
- An `if`, `match`, or block used as a statement needs no `;` unless it is
  the final expression of its block; keep `;` before a token that could
  continue an expression, as in `if c{...};-x`.
- A missing `else` is an empty `Unit` block.
- In a pattern, a bare identifier binds; enum variants are always qualified
  (`Shape::Dot`), so a variant is never mistaken for a binding.
- Imports must precede declarations (`E002`).

The grammar is LL(1) except for the place-or-expression decision in
statements, which the parser resolves by parsing an expression and
reinterpreting it as a place when `=` follows. Parsing is deterministic and
needs no symbol table, except that import aliases are known before
declarations so `alias::Name` can be read as a qualified type.
