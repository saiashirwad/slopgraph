# How a Rust CLI loads a TypeScript program

Research question: How can a Rust CLI load **one** TypeScript program from a `tsconfig.json` in 2026 and obtain an AST (and, if possible, symbols and types)? TypeScript 7.0 has no public compiler API. The tool language is Rust. Version 1 analyzes `.ts` and `.tsx` in one program.

Sources are official docs, source repos, and Microsoft TypeScript blog/API pages. Dates in URLs reflect crawl time; claims below point at those pages.

---

## 1. TypeScript 6.x public compiler API (JavaScript)

**What it provides:** parse, program, symbols, and types.

The TypeScript wiki documents the compiler API as:

- a `Program` (the whole application)
- a `CompilerHost` (filesystem)
- many `SourceFile`s (text + AST)

See [Using the Compiler API](https://github.com/Microsoft/TypeScript-wiki/blob/main/Using-the-Compiler-API.md).

`createProgram` builds a compilation unit from root names and options. Creating a program expands the set by following imports and triple-slash references. The `Program` exposes `getSourceFiles()`, `getTypeChecker()`, diagnostics, and emit. See [src/compiler/program.ts](https://github.com/Microsoft/TypeScript/blob/main/src/compiler/program.ts) and the wiki samples.

Microsoft’s TypeScript VFS page states you can create a `Program` as an entry point to the compiler API, emit JS/d.ts, and use the language service: [typescriptlang.org/dev/typescript-vfs](https://www.typescriptlang.org/dev/typescript-vfs/).

**How it reads `tsconfig.json`:**

Handbook: a `tsconfig.json` marks the project root, lists root files and compiler options. `tsc` with no input files searches for `tsconfig.json`. Input files on the command line ignore `tsconfig.json`. Include/files/exclude control the file set. [tsconfig-json handbook](https://www.typescriptlang.org/docs/handbook/tsconfig-json.html).

API: `readConfigFile` + `parseJsonConfigFileContent` parse config and expand `include`/`exclude`/`files` via a `ParseConfigHost` (`readDirectory`). This is the documented config path (used throughout the ecosystem; TypeScript source and [microsoft/TypeScript#62884](https://github.com/microsoft/TypeScript/issues/62884) discuss `parseJsonConfigFileContent` and `include`).

Path aliases: `compilerOptions.paths` and `baseUrl` are part of module resolution inside `createProgram` (handbook module resolution / path mapping).

**Rust constraint:** this API is in-process JavaScript (`typescript` npm package). A Rust CLI would need Node/Deno or another JS runtime. It is not a native Rust crate.

**TypeScript 6.0 role:** last JS-based compiler line; 7.0 is the Go port. [Announcing TypeScript 6.0](https://devblogs.microsoft.com/typescript/announcing-typescript-6-0/).

---

## 2. TypeScript 7.0 / tsgo (Go native port)

**What it provides internally:** full program, parse, type resolution, type checking (same files/resolution/types as TS 6.0 per project README).

[microsoft/typescript-go README](https://github.com/Microsoft/typescript-go):

| Feature | Status |
| Program creation | done (same files and module resolution as TS 6.0) |
| Parsing/scanning | done |
| Command line and `tsconfig.json` parsing | done |
| Type resolution / type checking | done |

Preview CLI: `npx tsgo` (later `tsc` in the `typescript` package). [Announcing TypeScript Native Previews](https://devblogs.microsoft.com/typescript/announcing-typescript-native-previews/), [Announcing TypeScript 7.0](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/).

**Public embed API:** not available as a stable library for other languages.

- Compiler implementation lives under `internal/` packages, e.g. [internal/compiler/program.go](https://github.com/microsoft/typescript-go/blob/main/internal/compiler/program.go), [internal/tsoptions/tsconfigparsing.go](https://github.com/microsoft/typescript-go/blob/main/internal/tsoptions/tsconfigparsing.go). Go `internal` packages are not importable from other modules.
- Daniel Rosenwasser (FAQ): the JS compiler API is **not** all ported. API consumers are expected to use **message passing / IPC**, not in-process calls. A curated API is planned for linting, transforms, resolution, language service. [Discussion #455](https://github.com/microsoft/typescript-go/discussions/455).
- Jake Bailey: public Go API is unlikely as a first-class product; main API users are JS or **out-of-proc**. [Discussion #481](https://github.com/microsoft/typescript-go/discussions/481).
- Issue on transformers/compiler API: Go has no good plugin story; embedding via Go plugins is not the solution. [typescript-go#516](https://github.com/microsoft/typescript-go/issues/516).

**How it reads `tsconfig.json`:** CLI parses config the same way as TS 6 (`tsconfig` parsing marked done). File set and path mapping are inside the Go compiler, not a public Rust/Go library surface.

**Rust CLI:** you can spawn `tsc`/`tsgo` as a process. You cannot import a typed Program/AST/checker from TypeScript 7 into Rust today. Third-party Go shims (e.g. ttsc driver) are not Microsoft public API.

**Oxlint’s type-aware path:** Oxlint (Rust) does **not** type-check itself. `tsgolint` (Go) builds TypeScript programs with typescript-go and returns diagnostics. [oxc.rs type-aware linting](https://oxc.rs/docs/guide/usage/linter/type-aware.html), [oxc-project/tsgolint](https://github.com/oxc-project/tsgolint). That is a Go sidecar, not a Rust type checker.

---

## 3. oxc (Rust)

### Parser / AST

[docs.rs/oxc_parser](https://docs.rs/oxc_parser/latest/oxc_parser/): full TypeScript, JSX, TSX. API: `Parser::new(&allocator, &source_text, source_type).parse()`. AST in `oxc_ast`. Parser does **not** do scope binding or symbol resolution.

### Symbols (binding), not TypeScript types

[docs.rs/oxc_semantic](https://docs.rs/oxc_semantic/latest/oxc_semantic/): semantic analysis = scopes, symbol table, references, early errors. `SemanticBuilder` walks a **single-file** AST. Docs: “Scope binding, symbol resolution and complicated syntax errors are not done in the parser, they are delegated to the semantic analyzer.” That is **JavaScript/TS binding**, not the TypeScript type checker.

Oxc’s own type-aware linting uses **typescript-go via tsgolint**, not `oxc_semantic` types. [Type-aware linting](https://oxc.rs/docs/guide/usage/linter/type-aware.html).

### `tsconfig.json` include/exclude and path aliases

[oxc_resolver](https://github.com/oxc-project/oxc-resolver) (Rust port of enhanced-resolve, tsconfig-paths, tsconfck):

- `tsconfig.extends`
- `compilerOptions.paths`
- `tsconfig.references`
- `${configDir}`
- discovery of which tsconfig applies to a file

`TsConfig` has `files`, `include`, `exclude`, `extends`, `compiler_options`, `references`. [docs.rs TsConfig](https://docs.rs/oxc_resolver/latest/oxc_resolver/struct.TsConfig.html).

`find_tsconfig`: walk ancestors; if the path is not in `files`/`include`/`exclude`, search project references. [ResolverImpl::find_tsconfig](https://docs.rs/oxc_resolver/latest/oxc_resolver/struct.ResolverImpl.html).

`resolve_file` / auto discovery: given a file, resolve aliases with that file’s tsconfig. [PR #860](https://github.com/oxc-project/oxc-resolver/pull/860).

Oxlint import plugin: discovers `tsconfig.json` for `compilerOptions.paths`. [multi-file analysis](https://oxc.rs/docs/guide/usage/linter/multi-file-analysis.html).

**Gap:** oxc_resolver tells you **which files belong** and **how to resolve imports**. It does not type-check. There is no oxc API that returns a TypeScript `Program` + `TypeChecker`.

---

## 4. swc (Rust)

**What it provides:** parse TypeScript/TSX to AST; transform/strip types; compile. Not a TypeScript type checker.

- [swc.rs](https://swc.rs/): TypeScript/JavaScript compiler in Rust.
- Parser: `Syntax::Typescript`, `parse_typescript_module` — [typescript.rs example](https://github.com/swc-project/swc/blob/main/crates/swc_ecma_parser/examples/typescript.rs).
- High-level `Compiler::parse_js` — [rustdoc Compiler](https://rustdoc.swc.rs/swc/struct.Compiler.html).
- Identifier resolver (`resolver_with_mark`) is **hygiene/scope for transforms**, not TS types. [docs.rs/swc](https://docs.rs/swc/latest/swc/index.html).

**`tsconfig.json`:**

- `TsConfigResolver` implements path mapping from **caller-supplied** `base_url` and `paths`. It does not parse `include`/`exclude` or build a file set. [TsConfigResolver](https://rustdoc.swc.rs/swc_core/ecma/loader/resolvers/tsc/struct.TsConfigResolver.html).
- SWC’s own config is `.swcrc` (`jsc.parser`, `jsc.base_url`, `jsc.paths`). [Compilation](https://swc.rs/docs/configuration/compilation). That is not TypeScript program construction.

**Verdict:** parse-only (plus strip/emit). No types. Incomplete tsconfig program loading.

---

## 5. Biome (Rust)

**What it provides:**

- Parse TypeScript 5.9, JSX, TSX. [Language support](https://biomejs.dev/internals/language-support/).
- Lossless parser: [biome_js_parser::parse](https://docs.rs/biome_js_parser/latest/biome_js_parser/fn.parse.html) with `JsFileSource::ts()`.
- Semantic model + **partial type inference** for linting (e.g. promises), not a full TypeScript checker. Author of the work: inference for `noFloatingPromises`, not assignment-complete TS. [Biome Type Inference](https://arendjr.nl/blog/2025/05/biome-type-architecture/).
- Module graph tracks imports/exports and inferred types across files. [biome_module_graph](https://github.com/biomejs/biome/blob/main/crates/biome_module_graph/src/module_graph.rs). `.d.ts` support was explicitly incomplete when typeRoots landed. [PR #6097](https://github.com/biomejs/biome/pull/6097).

**`tsconfig.json`:**

- Resolver supports `baseUrl`. [PR #7263](https://github.com/biomejs/biome/pull/7263).
- `compilerOptions.typeRoots` (default `node_modules/@types`). [PR #6097](https://github.com/biomejs/biome/pull/6097).
- `paths` is **not** fully reliable: confirmed bug that `paths` is ignored. [biome#10607](https://github.com/biomejs/biome/issues/10607) (open, S-Bug-confirmed). Referenced-project `paths` also reported broken. [biome#7644](https://github.com/biomejs/biome/issues/7644).
- File membership is Biome’s own include/ignore (`biome.json`), not a documented “load one TS program from tsconfig include/exclude” API.

**Verdict:** AST yes; experimental/incomplete types; tsconfig path aliases not a complete substitute for TypeScript program loading. Public crates are parser/semantic internals of a product, not a small “createProgram” API.

---

## 6. Comparison (today)

| Tool | Language | AST | Binding symbols | TypeScript types | Load file set from tsconfig include/exclude | Path aliases |
| --- | --- | --- | --- | --- | --- | --- |
| TypeScript 6.x API | JS | yes (`SourceFile`) | yes (`TypeChecker` symbols) | yes | yes (`parseJsonConfigFileContent`) | yes (`paths`/`baseUrl` in program) |
| TypeScript 7 / tsgo | Go CLI | internal only | internal | internal | yes in CLI, **not** public library | yes in CLI |
| oxc_parser + oxc_semantic | Rust | yes | yes (per-file JS/TS bind) | **no** (use tsgolint sidecar for types) | via oxc_resolver TsConfig fields + find_tsconfig | yes (`oxc_resolver`) |
| swc | Rust | yes | transform hygiene only | **no** | **no** (`.swcrc`; caller lists files) | partial (`TsConfigResolver` if you pass paths) |
| biome | Rust | yes | yes | **partial inference**, not TS checker | Biome config, not full TS program | incomplete (`paths` bugs) |

---

## 7. What is missing for a typed call graph

A typed call graph needs, for each call site:

1. Callee resolution across modules (re-exports, `export *`, `paths` aliases, `node_modules`).
2. Type of the callee (overloads, methods on classes/interfaces, union/intersection, generics).
3. Distinction of type-only vs value imports.
4. Optional: `.d.ts` and lib types for third-party APIs.

| Need | Available in-process in Rust without TS 6 JS API |
| File list + aliases | oxc_resolver |
| Syntax AST for `.ts`/`.tsx` | oxc_parser or swc or biome |
| Local bindings (`const f = …; f()`) | oxc_semantic (same file) |
| Cross-file import edges | walk `ImportDeclaration` + oxc_resolver |
| Method calls (`obj.m()`) typed | **missing** without TypeScript checker |
| Overload / generic instantiation | **missing** |
| `import type` vs value for edges | syntax only; types missing for `typeof` tricks |
| Full Program + TypeChecker | TS 6 JS API, or tsgo **internal** / IPC (not shipped as Rust API) |

Microsoft states TS 7 API will be IPC and curated, not “every function”. Until that IPC exists and exposes checker queries, a Rust process cannot obtain official TypeScript types without spawning Node (TS 6) or a Go sidecar (tsgolint/typescript-go internals).

---

## 8. Recommended stack for slopgraph version 1

Version 1: one program, `.ts` and `.tsx` only, Rust CLI.

**Do this:**

1. **Program membership and aliases:** `oxc_resolver`
   - Parse `tsconfig.json` (`extends`, `files`/`include`/`exclude`, `paths`, `references`).
   - Enumerate root files from include/exclude (same fields the crate already stores).
   - Resolve each `from "…"` with `resolve` / `resolve_file` so aliases match TypeScript path mapping as implemented by oxc (enhanced-resolve + tsconfig-paths port).

2. **AST:** `oxc_parser` + `oxc_ast` with `SourceType` for `.ts` / `.tsx`.

3. **Local symbols:** `oxc_semantic::SemanticBuilder` per file (scopes, references). Use this for same-file calls and to attach names to import bindings.

4. **Graph:** syntactic call/import graph
   - nodes: files + declared functions/classes/methods (from AST)
   - edges: `import`/`export` after resolution; `CallExpression` / `NewExpression` where the callee is a resolved local or imported binding
   - do **not** claim TypeScript method/overload accuracy

**Do not use for v1:**

- TypeScript 7 in-process (no public API).
- SWC as the program loader (no include/exclude program).
- Biome as the program loader (`paths` incomplete; types not TS).
- Embedding TS 6 via Node unless you explicitly accept a JS runtime (gives real types, fights the “Rust CLI / no TS 7 API” constraint).

**Named gaps (v1):**

- No TypeScript `TypeChecker`: method calls, overloads, generic inference, `this` typing, interface merging.
- `oxc_semantic` symbols are per file, not a global typed symbol table.
- oxc_resolver is “enhanced-resolve + tsconfig-paths”, not a certified copy of every TS 7 resolution mode (typescript-go README: not all resolution modes supported even in tsgo). Treat alias/file-set as **best-effort TS-compatible**, not bit-identical.
- Project references: oxc can discover referenced tsconfigs; v1 is **one** program — do not merge multiple `references` into one graph unless specified later.
- `.js` / `.d.ts` / `node_modules` types: out of v1 scope if only `.ts`/`.tsx` roots are walked; imports into `.d.ts` will not be typed.

**Later (typed graph):** spawn **tsgolint / typescript-go IPC** when Microsoft or oxc publish a stable checker query protocol; or keep a TS 6 `createProgram` worker. Do not wait on a public in-process TS 7 Rust/Go library; official guidance is IPC and “unlikely” public Go API.

---

## 9. Addendum: TypeScript 7 unstable API and oxlint type-aware lint (checked 2026-08-20)

Microsoft still says TypeScript **7.0 does not ship a stable API**. A new API is planned for **7.1**. [Announcing TypeScript 7.0](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/).

There **is** an **unstable IPC API**:

- JS clients: `@typescript/native-preview/unstable/sync` and `unstable/async`.
- Transport: spawn `tsgo`, talk over stdio / RPC. Not an in-process Rust or Go library.
- The unstable `Checker` already has `getResolvedSignature` and `getTypeAtLocation`. Source: [api.ts](https://github.com/microsoft/typescript-go/blob/24fabe95/_packages/native-preview/src/api/async/api.ts).
- Microsoft’s design is “same backend, different clients” ([PR #711](https://github.com/microsoft/typescript-go/pull/711)). A Rust process can use the same protocol. Third-party crates exist (`corsa_bind_client`, experimental `tsgo-rust-ipc`). They are not Microsoft products.

**Oxlint type-aware lint** does not put a TypeChecker inside oxc:

- Oxlint (Rust) does file walk, config, syntax rules, report.
- `tsgolint` (Go) **embeds typescript-go**, runs type-aware rules, returns diagnostics. [ARCHITECTURE.md](https://github.com/oxc-project/tsgolint/blob/main/ARCHITECTURE.md), [type-aware docs](https://oxc.rs/docs/guide/usage/linter/type-aware.html), [stable announcement](https://oxc.rs/blog/2026-07-22-type-aware-linting-stable).
- `tsgolint` uses `Checker_getResolvedSignature` **inside its own rules**. It is not a general type-query API for other tools.

Implication for slopgraph: typed call edges are available **out of process** via the unstable tsgo API. They are not available by linking oxc or by calling tsgolint as a library.

---

## Source list

- https://github.com/Microsoft/TypeScript-wiki/blob/main/Using-the-Compiler-API.md
- https://www.typescriptlang.org/docs/handbook/tsconfig-json.html
- https://www.typescriptlang.org/dev/typescript-vfs/
- https://github.com/Microsoft/TypeScript/blob/main/src/compiler/program.ts
- https://devblogs.microsoft.com/typescript/announcing-typescript-6-0/
- https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/
- https://devblogs.microsoft.com/typescript/announcing-typescript-native-previews/
- https://github.com/Microsoft/typescript-go
- https://github.com/microsoft/typescript-go/discussions/455
- https://github.com/microsoft/typescript-go/discussions/481
- https://github.com/microsoft/typescript-go/issues/516
- https://docs.rs/oxc_parser/latest/oxc_parser/
- https://docs.rs/oxc_semantic/latest/oxc_semantic/
- https://docs.rs/oxc_resolver/latest/oxc_resolver/
- https://github.com/oxc-project/oxc-resolver
- https://oxc.rs/docs/guide/usage/linter/multi-file-analysis.html
- https://oxc.rs/docs/guide/usage/linter/type-aware.html
- https://github.com/oxc-project/tsgolint
- https://swc.rs/
- https://rustdoc.swc.rs/swc_core/ecma/loader/resolvers/tsc/struct.TsConfigResolver.html
- https://docs.rs/biome_js_parser/latest/biome_js_parser/fn.parse.html
- https://biomejs.dev/internals/language-support/
- https://github.com/biomejs/biome/pull/7263
- https://github.com/biomejs/biome/pull/6097
- https://github.com/biomejs/biome/issues/10607
