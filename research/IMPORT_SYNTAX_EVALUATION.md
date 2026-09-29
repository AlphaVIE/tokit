# Experimental import spelling

For one relative file dependency, canonical `import"math.tok";` and candidate `use"math.tok";` each measure five tokens under locally installed `cl100k_base` and six under `o200k_base`. They use 17 and 14 UTF-8 bytes respectively. A named form, `mod math="math.tok";`, measures seven tokens and 20 bytes in both encodings. This one-path source measurement does not predict model generation or repair cost.

The prototype uses `import"math.tok";` because the token counts tie and the verb directly describes file loading. This syntax and the current flat declaration namespace are experimental; a scoped module design needs separate semantic and benchmark work before choosing a canonical representation.
