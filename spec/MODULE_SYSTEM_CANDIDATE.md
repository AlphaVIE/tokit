# Experimental file modules

Each `.tok` file has a separate declaration scope. An entry file imports another
file with an explicit local alias:

```tok
import math="math.tok";
fn main()->i32{math::triple(7)}
```

The imported file marks every declaration intended for use by another file:

```tok
fn helper(n:i32)->i32{n*3}
pub fn triple(n:i32)->i32{helper(n)}
```

`fn`, `struct`, and `enum` are private by default. `pub` makes a declaration
accessible through a direct import alias. Constructors, types, enum variants,
and match patterns use that alias too: `math::Box<i32>`, `math::Box(1)`,
`math::Event::Ready`, and `math::Event::Number(n)`. A file's own declarations
are used without a prefix. A directly imported module may import another file,
but its alias does not become visible to the entry file. A file loaded twice in
a diamond graph has one identity while each importer chooses its own alias.

The entry file must provide `main` for `tok run` and `tok build`. `tok test`
discovers local `test_` functions in every loaded file, including private ones.
The checked program and AI index use entry-relative canonical names such as
`math::triple`, independent of the importer's alias. Diagnostics retain each
source file's path and position. The native backend receives the same resolved
program as the interpreter.

Imports must precede declarations. Files must remain inside the entry file's
directory after path resolution. A duplicate alias, duplicate import path,
alias/declaration collision, cycle, or path escape reports `E118`. Access to a
private imported declaration reports `E119`. Duplicate declarations within one
file report `E106`; identical private names in separate files are allowed.

This module representation is experimental. There is no re-export syntax,
package manager, separate compilation, module initialization, or stable binary
interface. The `pub` flag is enforced by checking and exposed in the
multi-file AI index alongside direct import edges.
