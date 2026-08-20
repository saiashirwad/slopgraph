Title: Which graph stack does version 1 use
Type: grilling
Status: resolved

## Question

How a Rust CLI loads a TypeScript program recommends oxc in Rust (syntax and bindings, no TypeScript types). How to build a TypeScript call graph recommends a typed must-edge graph via `getResolvedSignature`. Those two answers cannot both be the version 1 stack.

Choose one:

1. **Rust oxc only** — syntactic graph. Detectors must ignore untrusted edges (methods, callbacks, JSX). Precision stays high by dropping those edges, not by guessing.
2. **Typed graph in process** — TypeScript 6.x `Program` in a JS runtime. The CLI may not be a pure Rust binary in version 1.
3. **Rust CLI + type sidecar** — oxc for parse and module graph; typescript-go / tsgolint-style IPC for call edges. Version 1 is larger.

Read both research answers before you choose. This decision blocks detector predicates that assume typed calls.

## Answer

Version 1 is **Rust CLI + TypeScript 7 unstable IPC** (option 3).

- oxc loads the program (file set, path aliases) and parses `.ts` / `.tsx`.
- Call edges are **typed edges** from `tsgo` `getResolvedSignature` over the unstable stdio API.
- Do not embed TypeScript in Rust. Do not use tsgolint as a library (it only emits lint diagnostics).
- Do not use TypeScript 6.x in-process JS.
- The spec must name this API **unstable**. TypeScript 7.0 has no stable API. 7.1 may change the surface.
- If a call has no resolved signature, drop the edge (high precision).

Addendum: [how-a-rust-cli-loads-a-typescript-program.md](../research/how-a-rust-cli-loads-a-typescript-program.md) section 9.
