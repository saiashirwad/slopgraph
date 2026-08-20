# Spec for the slopgraph CLI

Label: wayfinder:map

## Destination

A written spec that an implementer can build from. The spec contains the problem, the domain language, the non-goals, the first eight shapes, the graphs those shapes need, CLI behavior, and one example report. The map does not implement the tool.

## Notes

Domain: slopgraph. Canonical words live in `CONTEXT.md`. Every session must consult `/grilling` and `/domain-modeling`. Use `/prototype` for Example report. Use `/research` for fact tickets.

Standing preferences:

- Plan. Do not implement the tool in this map.
- The tool is a CLI. It detects slop and prints a report. It does not rewrite code. A human or an agent may run the CLI and use the report as cleanup guidance. Agent hooks stay out of scope.
- In slop, a structure is not proof that the structure is right.
- Version 1 analyzes one TypeScript program (`.ts` and `.tsx`) from one `tsconfig.json`.
- Intended implementation language: Rust. Version 1 uses oxc for parse and modules, and TypeScript 7 **unstable IPC** (`tsgo` `getResolvedSignature`) for typed call edges. The spec does not design crates.
- High precision: miss some slop rather than emit weak findings.
- Do not reimplement local lint (comments, `any`, empty `catch`).
- First shapes: single-use chain, empty wrapper, false sharing, unreachable, near-duplicate, tramp data, type clone, unreaching test.
- Refer to tickets by name, never by bare number.

## Decisions so far

- [How a Rust CLI loads a TypeScript program](issues/01-how-a-rust-cli-loads-a-typescript-program.md) — oxc can parse and resolve a program in Rust; TypeScript 7 has no embed API; types need a sidecar or JS `Program`.
- [How knip and slp define reachability](issues/02-how-knip-and-slp-define-reachability.md) — existing tools walk import graphs from package and plugin entries; they do not find single-use call chains, tramp data, or unreaching tests.
- [How to build a TypeScript call graph](issues/03-how-to-build-a-typescript-call-graph.md) — high-precision call edges need `getResolvedSignature`; syntactic name match is not enough.
- [How to detect near-duplicate functions](issues/08-how-to-detect-near-duplicate-functions.md) — use slp’s two passes: 50-token window hash plus AST-kind hash (≥20 nodes).
- [Which graph stack does version 1 use](issues/09-which-graph-stack-does-version-1-use.md) — Rust CLI + TypeScript 7 unstable IPC; oxc for parse/modules; `tsgo` for typed call edges.
- [What a finding contains](issues/04-what-a-finding-contains.md) — shape, location, evidence; no remedy; human-text report only.
- [When a single-use call chain is a finding](issues/05-when-a-single-use-call-chain-is-a-finding.md) — ≥2 in-degree-1 typed functions, any files; skip exports unless `--include-exported`; no name/size heuristics.
- [What counts as an entry point](issues/06-what-counts-as-an-entry-point.md) — `package.json` plus index/main/cli; tests in the graph as callers and default roots; `--production` drops test roots.
- [Example report](issues/07-example-report.md) — human-text report grouped by file; ASCII path as evidence (variant C).
- [Predicates for the other seven shapes](issues/10-predicates-for-the-other-seven-shapes.md) — slop bias; empty wrapper includes exports; false sharing is one consumer group; tramp data is one hop; near-duplicate uses slp gates; type clone ≥3 fields; unreaching test is import + zero typed edges.
- [How version 1 treats tsconfig edges](issues/11-how-version-1-treats-tsconfig-edges.md) — resolve aliases via oxc_resolver; `.js` never findings; ignore project references.

## Not yet specified

(none — the way is clear)

## Out of scope

- Auto-fix and rewrite.
- Agent hooks and CI gates.
- A repo-level slop score as the main output.
- Local lint shapes (narrative comments, `as any`, empty `catch`).
- Languages other than TypeScript.
- Monorepo as many programs in version 1.
- Crate layout and other implementation structure.
- A stable TypeScript 7.1 API (version 1 uses the unstable IPC API).
- Embedding the TypeScript checker in-process in Rust.
- Using tsgolint as a type-query library.
- Suggested remedies or patches in the report.
- JSON report in version 1.
- Framework route catalogs (deferred by What counts as an entry point).
