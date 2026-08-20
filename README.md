# slopgraph

Slopgraph is a read-only CLI for finding TypeScript code that only looks suspicious when you can see how the whole program hangs together.

Point it at one `tsconfig.json`. Slopgraph builds a module graph from imports and exports, then a typed call graph from TypeScript's resolved signatures. It uses those graphs to find things a file-at-a-time lint rule cannot see: files no entry point reaches, helper chains with one caller per step, wrappers and parameters that only forward work, duplicated function and type shapes, exports that are shared in name only, and tests that import production code without ever reaching it.

Here, **slop** means code—often generated or accumulated—that costs more to maintain than it earns. A finding is a lead for a human, not an instruction to delete code. Slopgraph does not rewrite source, assign a score, or fail a run just because it found something.

## See it

```console
$ slopgraph .

src/pipeline.ts

SINGLE-USE CHAIN
subject: stepOne  (line 9)
runPipeline   (exported, not in chain)
     │
     ▼
stepOne  ←── finding
     │
     ▼
stepTwo
     │
     ▼
stepThree
```

The path is the evidence. `runPipeline` is shown so you can see where the chain starts, but the finding begins at `stepOne`: exported functions stay out of single-use chains by default.

Slopgraph prints findings grouped by file. No findings means no output.

## Install

Slopgraph needs Rust to build and the native TypeScript 7 compiler to resolve call targets.

```bash
npm install --global typescript@^7
cargo install --git https://github.com/saiashirwad/slopgraph
```

Check that both commands are available:

```bash
tsc --version
slopgraph --help
```

A project-local TypeScript installation works too. Point Slopgraph at its native compiler binary:

```bash
npm install --save-dev typescript@^7
SLOPGRAPH_TSGO="$PWD/node_modules/.bin/tsc" slopgraph .
```

A `tsgo` binary from `@typescript/native-preview` is also recognized. Slopgraph checks `SLOPGRAPH_TSGO` first, then looks for `tsgo` and `tsc` on `PATH`.

To build a checkout instead of installing it:

```bash
cargo build --release
./target/release/slopgraph path/to/tsconfig.json
```

## Run it

Pass either a `tsconfig.json` or the directory containing one:

```bash
slopgraph .
slopgraph packages/api
slopgraph packages/api/tsconfig.json
```

### Options

| Option                        | What changes                                                                                           |
| ----------------------------- | ------------------------------------------------------------------------------------------------------ |
| `--production`                | Stops treating tests as reachability roots. Test files remain in the graph and still count as callers. |
| `--include-exported`          | Allows exported functions to appear inside single-use chains.                                          |
| `--color auto\|always\|never` | Controls terminal styling. `auto` is the default and respects `NO_COLOR`.                              |

A successful analysis exits with status `0`, whether or not it found anything. Load, resolution, or compiler errors exit non-zero. The report is currently human text only; there is no JSON mode or autofix.

## What it looks for

| Finding              | Reported when                                                                                                                                                                                                                            |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Unreachable**      | A non-test file has no import path from a root, or a function in a reachable file has no typed call path from a root. Functions inside an already-unreachable file are suppressed.                                                       |
| **Single-use chain** | Two or more functions in sequence each have exactly one resolved caller. Exported functions are excluded unless `--include-exported` is set.                                                                                             |
| **Empty wrapper**    | A function body consists only of one resolved call, optionally returned. Wrappers already explained by a single-use chain are not reported twice.                                                                                        |
| **False sharing**    | Every importer of an exported symbol lives in the same directory. The symbol is public in syntax but may be local in practice.                                                                                                           |
| **Near-duplicate**   | Two differently named, non-trivial functions have the same AST shape and strongly overlapping token windows after identifiers and literals are normalized. The current gates are at least 20 AST nodes and confidence of at least `0.7`. |
| **Tramp data**       | A parameter is never read locally and is used only as a direct argument to one or more resolved calls.                                                                                                                                   |
| **Type clone**       | Two interfaces or object type aliases have the same field names, modifiers, and field types, with at least three fields and no inheritance/intersection link.                                                                            |
| **Unreaching test**  | A test imports a production module, but no typed call path starting in that test reaches a function in the imported module. Indirect calls count.                                                                                        |

The repository includes a [golden report containing all eight findings](tests/fixtures/full-report/report.golden.txt).

## How the graph is built

Slopgraph combines two views of the same TypeScript program:

```text
                         ┌─ imports, exports, entry points ─┐
tsconfig.json ── files ──┤                                  ├── findings with evidence paths
                         └─ TypeScript-resolved call edges ─┘
```

The module graph comes from parsed imports and exports. The call graph comes from the native TypeScript compiler's resolved signatures.

That distinction matters. Slopgraph does not connect a call to every function with the same name. For a call such as `save()`, it asks TypeScript which declaration that particular call resolved to. If the declaration is external, unsupported, or cannot be mapped back to a function in the current program, Slopgraph leaves the edge out instead of guessing.

## Where reachability starts

Production roots are discovered from:

- `main`, `bin`, and `exports` in the program root's `package.json`
- `index`, `main`, or `cli` files at the program root or directly under `src/`

Tests are additional roots by default. Slopgraph recognizes conventional `*.test.*` and `*.spec.*` names, `test.ts(x)` and `spec.ts(x)`, and files under `test`, `tests`, or `__tests__` directories. `--production` removes those test roots; it does not remove tests from the program.

There is no framework route catalog. A Next.js page, NestJS controller, test-runner hook, dependency-injection registration, or other convention-based entry is not automatically a root unless it is also reachable from one of the roots above.

## Scope and caveats

Slopgraph is deliberately narrow:

- It analyzes one `tsconfig.json` at a time. Project references are not merged; run it once per program in a monorepo.
- Findings are produced for `.ts`, `.tsx`, `.mts`, and `.cts` files. Declaration files and JavaScript files are ignored, even when `allowJs` is enabled.
- Path aliases, `baseUrl`, and extended tsconfigs are resolved on a best-effort basis. Imports that cannot be resolved do not create graph edges.
- Dynamic or framework-mediated control flow can be invisible to the call graph. Treat reachability findings around callbacks, registries, decorators, reflection, and dependency injection as prompts to inspect—not proof that code is dead.
- Slopgraph currently uses TypeScript 7's native compiler API over stdio. That interface is not a stable public API, so a compiler update may require a Slopgraph update. Pinning TypeScript is sensible for repeatable runs.

The bias is toward evidence you can inspect, not a universal definition of bad code.

## Development

```bash
cargo test
```

Integration tests need the native TypeScript compiler available through `SLOPGRAPH_TSGO`, `tsgo`, or `tsc`, just like the CLI.
