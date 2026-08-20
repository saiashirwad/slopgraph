# Slopgraph Specification, Version 1

The canonical vocabulary is in [CONTEXT.md](../../CONTEXT.md). This spec uses only that vocabulary.

## 1. Purpose

Slopgraph is a command-line tool.

It analyzes one TypeScript program. It finds graph-shaped slop. It prints a report of findings.

**Slop** is generated code that adds cost but does not add enough value. The report is cleanup guidance for a human or an agent. The tool does not change code. It only reports.

In slop, a structure is not proof that the structure is right.

## 2. Non-goals (Version 1)

The tool does not:

- rewrite or fix code
- run as an agent hook or a CI gate
- print a repo-level slop score as the main output
- analyze languages other than TypeScript
- analyze more than one program
- reimplement local lint shapes (narrative comments, `as any`, empty `catch`)
- print a JSON report

## 3. Input and program

The user runs the CLI with a path to a `tsconfig.json`, or to the directory that contains it.

The **program** is the set of files that the `tsconfig.json` includes. Only `.ts` and `.tsx` files are in the program.

Rules:

- Resolve `paths`, `extends`, and `baseUrl` with oxc_resolver.
- Ignore project references. Analyze the root program only. A referenced project is a separate program.
- `.js` and `.jsx` files are never findings.
- Edges into `.js` and `.jsx` files are dropped.

## 4. Stack

- Rust CLI.
- oxc_parser and oxc_semantic for parsing and local binding.
- oxc_resolver for tsconfig handling and module resolution.
- TypeScript 7 unstable IPC API (`tsgo`) for typed call edges.

The CLI spawns the `tsgo` process and communicates over stdio. It calls `getResolvedSignature` for each call-like node.

Precision rules:

- A call edge exists only when `getResolvedSignature` resolves it.
- If a call has no resolved signature, drop the edge.
- Never guess a call target by name.

The TypeScript 7 API is unstable. Version 1 accepts this. Version 1 does not use the TypeScript 6.x API in-process.

## 5. Graphs

Version 1 builds:

- **Module graph** — file nodes; import and export edges resolved by oxc_resolver.
- **Call graph** — function nodes; typed call edges from `tsgo`.
- **Consumer groups** — the importers of each export, grouped by directory.

## 6. Entry points and tests

**Entry points:**

- `package.json` fields `main`, `bin`, `exports`.
- Files named `index`, `main`, or `cli` at the repo root or under `src/`, with TypeScript extensions.

Framework routes are not entry points in version 1.

**Tests:**

- Test files are in the graph.
- Tests are callers. They add in-degree for the single-use chain shape.
- Tests are roots for the unreachable shape by default.
- The `--production` flag removes test roots.
- Tests stay in the graph under `--production` for the unreaching test shape.

## 7. Shapes and predicates

### 7.1 Single-use chain

A path of **two or more** functions where each function has **exactly one caller** on typed edges.

- The head that calls into the path may have many callers. Count only the in-degree-1 nodes.
- The path may cross files. There is no helper-file heuristic.
- Exported functions are not chain nodes by default. The `--include-exported` flag allows them.
- No name gate. No body-size gate.
- One in-degree-1 function whose body only forwards is an **empty wrapper**, not this shape.

### 7.2 Empty wrapper

A function whose body is only a forward on a typed edge. The forward may be `return f(x)`, `f(x)` with no return, or `return f(x) as T`.

- Exported functions are included.
- No in-degree requirement.

### 7.3 False sharing

An export with exactly **one consumer group**.

- The consumer group may be the declaring directory.
- There is no file-name gate.

### 7.4 Unreachable

A **file** with no import path from an entry point, or a **function** with no typed-edge path from an entry point.

- Tests are roots unless `--production`.

### 7.5 Near-duplicate

Two or more functions with distinct names and the same shape, found by two passes:

1. Token-window hash: windows of 50 normalized tokens. Identifiers normalize to `$ID`; literals normalize to `$LIT`.
2. AST-kind hash: exact kind sequence of a function body with at least 20 nodes.

Confidence must be at least 0.7.

### 7.6 Tramp data

A parameter that a function does not read locally, except as an argument to a call, on a typed call path.

- One intermediate function is enough.

### 7.7 Type clone

Two types with the same field names and the same field types, different type names, no `extends` link, and at least 3 fields.

### 7.8 Unreaching test

A test imports a production module, and no typed edge from that test reaches any function in that module.

## 8. Findings and report

A **finding** has:

- **shape** — the canonical shape name
- **location** — file and span of the subject
- **evidence** — proof specific to the shape (for a single-use chain: the function path)

A finding does not include a remedy or a patch.

The report is human text. Findings are grouped by file. Evidence is an ASCII path.

### 8.1 CLI flags (version 1)

- `--include-exported` — allows exported functions as single-use chain nodes.
- `--production` — removes test roots for the unreachable shape.

## 9. Example report

Fake program `src/orders.ts`:

```ts
export function handleOrder(order: Order) {
  prepareOrder(order);
}

function prepareOrder(order: Order) {
  validateAndSave(order);
}

function validateAndSave(order: Order) {
  db.save(order);
}

function persistOrder(order: Order) {
  return saveOrder(order);
}

function saveOrder(order: Order) {
  db.insert("orders", order);
}
```

Report:

```
src/orders.ts

SINGLE-USE CHAIN
subject: prepareOrder  (line 5)
handleOrder   (exported, not in chain)
     │
     ▼
prepareOrder  ←── finding
     │
     ▼
validateAndSave

EMPTY WRAPPER
subject: persistOrder  (line 13)
persistOrder  ←── finding
     │  return only
     ▼
saveOrder
```

The throwaway prototype is at [prototype/example-report.html](prototype/example-report.html).

## 10. Out of scope (later)

- Framework route catalogs as entry points.
- JSON report.
- Monorepo as many programs.
- Auto-fix and rewrite.
- Agent hooks and CI gates.
- Suggested remedies in the report.
