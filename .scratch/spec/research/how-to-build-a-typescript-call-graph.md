# How to build a TypeScript call graph

Primary sources only. Date of search: 2026-03-25.

## Sources

- TypeScript Compiler API wiki: https://github.com/microsoft/TypeScript/wiki/Using-the-Compiler-API
- TypeScript issue #20051 (`getResolvedSignature` vs `getSymbolAtLocation` on `CallExpression`): https://github.com/Microsoft/TypeScript/issues/20051
- TypeScript issue #5218 (Ryan Cavanaugh: `getSymbolAtLocation` is for Go to Definition): https://github.com/Microsoft/TypeScript/issues/5218
- TypeScript PR #27627 (JSX checked as calls; `getResolvedSignature` for JSX): https://github.com/Microsoft/TypeScript/pull/27627
- TypeScript PR #59933 (`JsxOpeningFragment` is `CallLikeExpression`; `isCallLikeExpression` also covers `InstanceOfExpression`): https://github.com/microsoft/TypeScript/pull/59933
- TypeScript PR #60036 (call-like set: calls, decorators, tagged templates, JSX tags): https://github.com/microsoft/TypeScript/pull/60036
- TypeScript JSX handbook (JSX emit is factory calls): https://www.typescriptlang.org/docs/handbook/jsx.html
- ts-morph TypeChecker (`getResolvedSignature` on call-like nodes): https://ts-morph.com/navigation/type-checker
- Oxc parser/semantic (AST, scopes, symbols, CFG; no type checker / no call graph): https://oxc.rs/docs/learn/architecture/parser , https://docs.rs/oxc/latest/oxc/semantic/struct.Semantic.html , https://github.com/oxc-project/oxc/blob/main/crates/oxc_semantic/src/lib.rs
- Oxlint type-aware linting (syntax in Rust; types via tsgolint + typescript-go): https://oxc.rs/docs/guide/usage/linter/type-aware.html
- code2flow README (syntactic AST call graph for dynamic languages): https://github.com/scottrogowski/code2flow/blob/master/README.md

## 1. Syntactic edges vs typed edges

### Syntactic graph

code2flow states the algorithm:

1. Build an AST.
2. Find function definitions.
3. Find call sites.
4. Connect names.

code2flow also states: no algorithm can make a perfect call graph for a dynamic language. The JS path uses Acorn (parse only). Edges come from name match and in-scope heuristics, not types.

Oxc `Semantic` gives:

- AST
- scopes and bindings (`BindingIdentifier` vs `IdentifierReference`)
- symbol table and references
- optional CFG

Oxc does **not** resolve call targets by type. A syntactic edge is: callee name + local binding, or a same-name guess across files.

Trust for syntactic edges:

- Direct call of a locally bound function: `function f(){}; f()`
- Direct call of an imported binding if you also resolve the module specifier (Oxc does not do TS module resolution by itself)
- Nested function calls inside the same file when the callee is a `BindingIdentifier` in an enclosing scope

Do not trust syntactic name match for:

- Methods (`obj.m()`, `this.m()`)
- Functions passed as values
- Anonymous functions / lambdas (code2flow skips these)
- Same name in two namespaces (code2flow skips to avoid ambiguity)
- Imports from outside the project that share a local name
- `eval`, computed property access, `apply`/`call`/`bind` as dynamic dispatch

### Typed graph

TypeScript `Program` + `TypeChecker` is the primary source for typed edges.

Wiki: `Program` is the whole application; `SourceFile` is the AST; `createProgram` uses a `CompilerHost`.

For a call-like node:

```ts
const signature = checker.getResolvedSignature(callLike);
const declaration = signature?.declaration;
```

This is the official answer on microsoft/TypeScript#20051 (ajafff). `getSymbolAtLocation` on the callee identifier is Go to Definition (Ryan Cavanaugh, #5218), not overload-resolved call target.

Also follow aliases:

```ts
const symbol = checker.getSymbolAtLocation(expr);
const original = symbol && (symbol.flags & ts.SymbolFlags.Alias)
  ? checker.getAliasedSymbol(symbol)
  : symbol;
const declarations = original?.getDeclarations();
```

Typed edge = call-like node → resolved signature → declaration(s).

This needs a full TS program (`tsconfig`, module resolution, `lib`). Oxlint documents the same split: syntax in Oxc, types in typescript-go / tsgolint.

## 2. Call-like forms in TypeScript

`CallLikeExpression` (compiler types + PR #59933 / #60036) includes more than `CallExpression`:

| Syntax | Node | Typed resolution |
| --- | --- | --- |
| `f(x)` | `CallExpression` | `getResolvedSignature` |
| `new C(x)` | `NewExpression` | same |
| `` tag`tpl` `` | `TaggedTemplateExpression` | same |
| `@dec` | decorator | same (call-like) |
| `<Foo />` / `<Foo>` | `JsxOpeningLikeElement` | same (PR #27627) |
| `<>` | `JsxOpeningFragment` | call-like via fragment factory (PR #59933) |
| `x instanceof C` | `InstanceOfExpression` | `isCallLikeExpression` (PR #59933); **not** a user call |

`isCallLikeExpression` is **not** equal to “user function call”. Do not put `instanceof` on the product call graph.

### Methods

Typed: `getResolvedSignature` on `obj.m()` / `this.m()` / `super.m()` uses the type of the receiver and overload resolution. `getSymbolAtLocation(call.expression)` can point at the interface/class method symbol (Stack Overflow / Sherret pattern).

Syntactic: `m` as a name is not unique. Do not trust name-only method edges.

If the method is on a union or interface with many implementers, the signature declaration may be the interface method, not one class body. A high-precision detector must record that as an interface/target-set, not a single concrete function, unless the receiver type is a single class.

### Callbacks and function values

Passing `f` as a value is not a call. A call graph edge exists only at the later `CallExpression` (or JSX handler invoke).

Typed checker can still resolve `cb()` if `cb` has a call signature. The declaration may be a type-level signature (`type Fn = () => void`) with no body. High precision: only emit an edge when `signature.declaration` is a function-like **value** in project source. Ignore lib `.d.ts` call signatures unless the product wants “calls Array.map”.

Higher-order inference: `signature.declaration` may be a synthetic/instantiated signature without the original function’s type parameters (microsoft/TypeScript#30296). Do not assume `declaration` is the user function you expect for `pipe(list, box)(...)`.

code2flow: renamed functions and functions passed as parameters are skipped.

### `import()`

Dynamic `import("mod")` is a `CallExpression` whose callee is the `import` keyword. The resolved signature is the TS `ImportCall` / Promise-of-module API, **not** a user function.

Do not treat `import()` as a call-graph edge to module exports.

Module graph is separate: `checker.getSymbolAtLocation(moduleSpecifier)` / `getAliasedSymbol` (Sherret). That is an import edge, not a call edge.

If the program later calls a property of the awaited namespace (`(await import("./m")).f()`), that later `CallExpression` is the call edge.

### JSX handlers and tags

Two different facts:

1. **JSX tag** `<Foo />` is a call-like expression. TS checks it with the same `getResolvedSignature` path as calls (PR #27627). Emit is `React.createElement` / `_jsx` (handbook). A typed call-graph **may** add an edge from the enclosing function to `Foo` (function component) or to the class constructor / factory. Intrinsic tags (`<div>`) resolve to JSX namespace types in `lib` / React types — ignore for a product graph of user code.

2. **JSX handler** `<button onClick={handler} />` is **not** a call of `handler`. It is an attribute whose value is a function. The call happens in the runtime (React). A sound static graph must **not** add `enclosingFn → handler` as a call unless you define a separate “registers callback” edge kind.

Typed tools can still **identify** `handler` via `getSymbolAtLocation` / contextual type of the attribute (`getContextualType`). That is identity of a value, not a call.

## 3. What a high-precision detector can trust vs must ignore

### Trust (typed, after `createProgram` + diagnostics you accept)

- `CallExpression` / `NewExpression` / `TaggedTemplateExpression` where `getResolvedSignature` returns a signature whose `declaration` is a single function-like in **project** source (not `undefined`, not only `.d.ts`).
- Direct identifier callees whose symbol aliases to one function declaration (`getAliasedSymbol` + `getDeclarations().length === 1`).
- Method calls where the receiver’s apparent type has one call/construct signature pointing at one class method body.
- JSX **user** components (`JsxOpeningLikeElement` whose tag is a function/class in project source), as call-like, labeled as JSX-call not JS-call if you need the distinction.

### Ignore or mark untrusted (do not use as high-precision call edges)

- Name-only matches (code2flow style) for methods, imports, or duplicate names
- `getResolvedSignature` missing, or `declaration` missing (untyped JS, `any`, error calls — #20051 needed a clean program)
- Signatures whose only declarations are in `lib` / `@types` (unless you opt in to “calls into node_modules”)
- Union/overload sets with many declarations — at most a **may-call** set, not a must-edge
- Function values, callbacks, `.then`, event emitters, `setTimeout(fn)` — the argument is not invoked at that node
- JSX `onClick={fn}` and similar props
- `import()`, `require` as module loaders
- `instanceof` (call-like in TS, not a call)
- Decorators unless v1 explicitly wants decorator invocation
- Computed callees (`obj[k]()`, `fns[i]()`), `Function.prototype.call/apply/bind`, `new Proxy`, `eval`
- Higher-order instantiated signatures where `declaration` is not the original user function (#30296)
- Watch-mode stale `getSymbolAtLocation` (microsoft/TypeScript#59270) — rebuild `Program` for batch analysis

### Oxc role

Use Oxc (or TS parse) to **find** call-like nodes and function bodies fast. Use TypeScript (or typescript-go) to **resolve** targets. Oxlint already splits this way. Do not ship “Oxc call graph” as typed.

## 4. Version 1: typed, syntactic, or both

**Ship a typed graph as the product graph. Keep a syntactic layer only as a finder, not as truth.**

Reasons from the sources:

- code2flow documents that syntactic graphs for JS are estimates and skip methods, callbacks, and name collisions — the cases TypeScript programs use constantly.
- TS already classifies the real call-like set and resolves overloads/JSX via `getResolvedSignature`.
- Oxc semantic is necessary for speed and scopes but is not a type checker; Oxlint does not pretend otherwise.

v1 practical design (sound, not complete):

1. `ts.createProgram` from the project `tsconfig` (or typescript-go equivalent).
2. Walk each `SourceFile`; collect `ts.isCallLikeExpression` nodes **except** `instanceof`.
3. For each remaining node, `checker.getResolvedSignature`.
4. If `declaration` is one project function-like, emit a **must** call edge from enclosing function-like to that declaration.
5. If several declarations or only `.d.ts`, emit nothing in the high-precision set (or a separate `may` channel, not mixed).
6. Do not emit edges for `import()`, JSX handlers, or callback arguments.
7. Optionally record JSX tags as `jsx-call` edges when the tag resolves to a project component.

Do **not** ship a merged syntactic+typed graph in v1. Name-based false edges would poison a high-precision detector. A second, clearly labeled `syntactic-guess` graph can wait.

Caveats the sources force:

- Typed v1 needs a compilable-enough program; 22k diagnostics made `declaration` undefined (#20051).
- Compiler API wiki: TypeScript 7.1 will have a different API; tsgolint already tracks TS 7 / typescript-go.
- Completeness is impossible for JS/TS (code2flow). Precision is the v1 goal.

## 5. Minimal typed walk (Compiler API)

```ts
import * as ts from "typescript";

function enclosingFunction(node: ts.Node): ts.FunctionLikeDeclaration | undefined {
  while (node) {
    if (ts.isFunctionLike(node) && !ts.isFunctionTypeNode(node)) {
      return node as ts.FunctionLikeDeclaration;
    }
    node = node.parent;
  }
}

function walk(sf: ts.SourceFile, checker: ts.TypeChecker, add: (from: ts.Node, to: ts.Node) => void) {
  function visit(node: ts.Node) {
    if (ts.isCallLikeExpression(node) && !ts.isBinaryExpression(node)) {
      const sig = checker.getResolvedSignature(node);
      const decl = sig?.declaration;
      const from = enclosingFunction(node);
      if (from && decl && !decl.getSourceFile().isDeclarationFile) {
        add(from, decl);
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(sf);
}
```

`isBinaryExpression` here is a stand-in to skip `instanceof` (call-like after PR #59933). Prefer the public `ts.isInstanceOfExpression` when the TS version exports it.

Filter JSX: `ts.isJsxOpeningLikeElement` / `ts.isJsxOpeningFragment` if v1 does not want JSX-as-call.

## 6. Answers to the ticket questions

| Question | Answer from sources |
| --- | --- |
| Syntactic vs typed | Syntactic = AST name/scope (code2flow, Oxc). Typed = `Program` + `getResolvedSignature` (TS). |
| Methods | Typed only; syntactic names are wrong. |
| Callbacks / function values | Not calls at the pass site. Resolve later call of the value if types allow; else ignore. |
| `import()` | Module load, not a user call. |
| JSX handlers | Not calls. JSX **tags** are call-like in the checker. |
| High precision trusts | Single project `signature.declaration` on real call-like nodes. |
| High precision ignores | Name match, `any`, unions, lib, callbacks, JSX handlers, `import()`, `instanceof`, dynamic dispatch. |
| v1 | Typed must-edges only. Syntactic finder underneath. Do not mix guess edges into the product graph. |
