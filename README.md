# Slopgraph

A CLI that detects graph-shaped slop in a TypeScript program and prints a report of findings.

The tool analyzes one TypeScript program (`.ts` / `.tsx` from one `tsconfig.json`), builds a module graph and a typed call graph, and finds eight shapes: single-use chain, empty wrapper, false sharing, unreachable, near-duplicate, tramp data, type clone, unreaching test.

## Status

Version 1 is specified at [`.scratch/spec/spec.md`](.scratch/spec/spec.md). The CLI loads one TypeScript program and reports **false sharing**. Other shapes are not implemented yet.

## Usage

```
slopgraph path/to/tsconfig.json
slopgraph path/to/project
```

The path is a `tsconfig.json`, or the directory that contains it. The report is human text: findings grouped by file, evidence as an ASCII path. The tool does not change code.

## Documents

- [`.scratch/spec/spec.md`](.scratch/spec/spec.md) — the version 1 spec
- [`CONTEXT.md`](CONTEXT.md) — canonical vocabulary
- [`.scratch/spec/map.md`](.scratch/spec/map.md) — the wayfinder map (planning decisions)

## Issue tracker

Issues are tracked on GitHub. The wayfinder map lives at the issue labelled `wayfinder:map`.
