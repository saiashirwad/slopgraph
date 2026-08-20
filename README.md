# slopgraph

Slopgraph is a read-only CLI that finds TypeScript code that only looks suspicious when you can see the whole program: files no entry point reaches, helper chains with one caller per step, wrappers and parameters that only forward work, duplicated function and type shapes, exports that are shared in name only, and tests that never reach the production code they import.

A finding is a lead for a human, not a verdict. Slopgraph never rewrites source, assigns a score, or fails a run for finding something.

## Install

Slopgraph needs Rust to build and the native TypeScript 7 compiler to resolve call targets:

```bash
npm install --global typescript@^7
cargo install --git https://github.com/saiashirwad/slopgraph
```

It checks `SLOPGRAPH_TSGO` first, then `tsgo` and `tsc` on `PATH` (`tsgo` comes from `@typescript/native-preview`). A project-local TypeScript works too:

```bash
npm install --save-dev typescript@^7
SLOPGRAPH_TSGO="$PWD/node_modules/.bin/tsc" slopgraph .
```

## Run

```bash
slopgraph .                     # or a path to a tsconfig.json / its directory
slopgraph --production          # tests are not reachability roots
slopgraph --include-exported    # exported functions may appear in single-use chains
slopgraph --color never         # auto | always | never; auto respects NO_COLOR
```

Findings print grouped by file with evidence paths; no findings means no output. A run exits `0` whether or not it found anything; load, resolution, or compiler errors exit non-zero. Output is human text only — no JSON mode or autofix.

## Findings

| Finding | Reported when |
| --- | --- |
| **Unreachable** | A non-test file or function has no path from a root. |
| **Single-use chain** | Two or more functions in sequence each have exactly one resolved caller. |
| **Empty wrapper** | A body is only one resolved call, optionally returned. |
| **False sharing** | Every importer of an exported symbol lives in one directory. |
| **Near-duplicate** | Same AST shape and strongly overlapping tokens (≥20 nodes, ≥0.7 confidence). |
| **Tramp data** | A parameter is never read locally, only passed on. |
| **Type clone** | Two interfaces or type aliases with identical fields (≥3) and no inheritance link. |
| **Unreaching test** | A test imports a module but no call path reaches it. |

[Golden report with all eight findings](tests/fixtures/full-report/report.golden.txt)

## How it works

Slopgraph parses one `tsconfig.json` into a module graph (imports, exports, entry points) and a typed call graph from TypeScript 7's resolved signatures. It never matches calls by name: each call is resolved to the declaration it actually targets, and unresolvable edges are left out rather than guessed.

Production roots come from `main`, `bin`, and `exports` in `package.json`, plus `index`/`main`/`cli` files at the root or under `src/`. Conventional test files are extra roots unless `--production` is set. There is no framework route catalog: Next.js pages, NestJS controllers, DI registrations, and the like are roots only if reachable.

## Caveats

- One `tsconfig.json` at a time; project references are not merged.
- `.ts`, `.tsx`, `.mts`, `.cts` only; declarations and JavaScript are ignored.
- Path aliases and extended tsconfigs resolve on a best-effort basis.
- Dynamic control flow (callbacks, decorators, reflection, DI) can be invisible to the call graph.
- TypeScript 7's native compiler API over stdio is not stable; a compiler update may require a Slopgraph update.

## Development

```bash
cargo test
```

Integration tests need the native compiler via `SLOPGRAPH_TSGO`, `tsgo`, or `tsc`.
