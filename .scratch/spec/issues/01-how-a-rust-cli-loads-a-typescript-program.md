Title: How a Rust CLI loads a TypeScript program
Type: research
Status: resolved

## Question

How can a Rust CLI load one TypeScript program from a `tsconfig.json` in 2026 and obtain an AST (and, if possible, symbols and types)?

TypeScript 7.0 has no public compiler API. The intended tool language is Rust. Version 1 analyzes `.ts` and `.tsx` files in one program.

Report, from primary sources:

- What oxc, swc, biome, tsgo, and the TypeScript 6.x API can each provide today (parse only, or types too).
- How each tool reads `tsconfig.json` include/exclude and path aliases.
- What is missing for a typed call graph.
- A recommended stack for slopgraph version 1, with the gaps named.

## Answer

Findings: [how-a-rust-cli-loads-a-typescript-program.md](../research/how-a-rust-cli-loads-a-typescript-program.md)

A Rust process can parse TypeScript with oxc and resolve `tsconfig` paths with oxc_resolver. It cannot embed TypeScript 7: that compiler has no public in-process API. Types today mean TypeScript 6.x in a JS runtime, or a Go sidecar (`tsgolint` / typescript-go) over IPC.

The research stack for a Rust-only v1 is oxc_resolver + oxc_parser + oxc_semantic (syntax and bindings, not TypeScript types). A typed call graph is a gap. This conflicts with How to build a TypeScript call graph, which says high-precision call edges need `getResolvedSignature`.
