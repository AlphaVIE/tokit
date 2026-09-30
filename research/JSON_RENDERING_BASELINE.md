# JSON module seed measurements

The formatted [Tokit JSON module](../examples/json/json.tok) is a workload
seed for future equivalent-language comparisons. Measured with
scripts/token_cost.py, tiktoken 0.14.0, and the checked tok stats command:

| Stage | UTF-8 bytes | AST nodes | Semantic operations | cl100k_base | o200k_base |
| --- | ---: | ---: | ---: | ---: | ---: |
| Renderer only (`a688589`) | 2599 | 540 | 165 | 836 | 852 |
| Parser and renderer | 8750 | 1757 | 628 | 2773 | 2810 |

These counts cover the module alone. They omit the importing entry file,
tests, prompts, compiler feedback, and repairs. They do not establish
token efficiency against another language or model family. The parser
copies byte arrays between calls, while the renderer copies arrays and
repeatedly concatenates strings. Native time and allocation behavior
remain unmeasured. Add a semantically equivalent corpus and measured
runtime comparison before any cross-language or performance claim.
