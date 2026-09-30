# JSON renderer seed measurement

The formatted [Tokit JSON module](../examples/json/json.tok) is a workload
seed for future equivalent-language comparisons. Measured with
scripts/token_cost.py, tiktoken 0.14.0, and the checked tok stats command:

| Source | UTF-8 bytes | AST nodes | Semantic operations | cl100k_base | o200k_base |
| --- | ---: | ---: | ---: | ---: | ---: |
| json.tok | 2599 | 540 | 165 | 836 | 852 |

These counts cover the module alone. They omit the importing entry file,
tests, prompts, compiler feedback, and repairs. They do not establish
token efficiency against another language or model family. The renderer
copies arrays and repeatedly concatenates strings; native time and
allocation behavior remain unmeasured. Add a semantically equivalent
parser and serializer corpus before any cross-language or performance
claim.
