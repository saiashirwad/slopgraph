Title: How to build a TypeScript call graph
Type: research
Status: resolved

## Question

What is a sound, practical way to build a call graph for a TypeScript program?

Primary sources: TypeScript checker APIs (6.x), oxc, existing call-graph tools (code2flow and any typed TS call-graph library).

Report:

- Syntactic vs typed edges.
- Methods, callbacks, function values, dynamic `import()`, and JSX handlers.
- What a high-precision detector can trust vs what it must ignore.
- Whether version 1 should ship a typed graph, a syntactic graph, or both.

## Answer

Findings: [how-to-build-a-typescript-call-graph.md](../research/how-to-build-a-typescript-call-graph.md)

High-precision call edges come from TypeScript `Program` + `checker.getResolvedSignature` on call-like nodes. `getSymbolAtLocation` is Go to Definition, not the call target.

A syntactic name graph (oxc, code2flow) is a finder only. Do not trust it for methods, callbacks, JSX handlers, `import()`, `call`/`apply`, or duplicate names.

The research recommendation is a typed must-edge graph for version 1. That conflicts with How a Rust CLI loads a TypeScript program (Rust oxc has no TypeChecker).
