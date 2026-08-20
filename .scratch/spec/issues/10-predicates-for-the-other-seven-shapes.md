Title: Predicates for the other seven shapes
Type: grilling
Status: resolved

## Question

Using the style from When a single-use call chain is a finding (typed facts, no name heuristics, high-precision default, opt-in flags to loosen), what is the predicate for each remaining shape?

Shapes: empty wrapper, false sharing, unreachable, near-duplicate, tramp data, type clone, unreaching test.

Near-duplicate already has a research method (slp two-pass). Decide only the precision gates, not a new algorithm.

## Answer

Bias: this is slop. A structure is not proof that it is right. The report is cleanup guidance for a human or an agent. The CLI does not rewrite. Agent hooks stay out of scope.

- **Empty wrapper** — body is only a forward on a typed edge (optional `return`, optional `as T`). Exported functions are included.
- **False sharing** — an export with exactly one consumer group, including the declaring directory.
- **Unreachable** — a file with no import path from entry points, or a function with no typed-edge path from entry points. Tests are roots unless `--production`.
- **Near-duplicate** — slp two-pass: 50-token window hash (`$ID`/`$LIT`) plus AST-kind hash (≥20 nodes), distinct names, confidence ≥ 0.7.
- **Tramp data** — a parameter not read locally except as an argument, in **one or more** functions on a typed call path.
- **Type clone** — same field names and field types, different type names, no `extends`, at least 3 fields.
- **Unreaching test** — the test imports a production module, and no typed edge from that test reaches any function in that module.
