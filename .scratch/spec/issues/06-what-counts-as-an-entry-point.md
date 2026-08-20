Title: What counts as an entry point
Type: grilling
Status: resolved
Blocked by: 02

## Question

What is an entry point in version 1, and how do tests sit in the graph?

Decide:

- Roots: `package.json` `bin` / `exports` / `main` only, or also framework routes.
- Whether test files are in the program graph.
- Whether a test is a root for unreachability, a caller for single-use chain, both, or neither.
- How this supports the unreaching test shape.

## Answer

**Production roots (version 1):** `package.json` `main` / `bin` / `exports`, plus files named `index`, `main`, or `cli` at the repo root or under `src/` (TypeScript extensions). No framework route catalog.

**Tests:** Test files are in the graph.

- They are **callers** for **single-use chain** (they add in-degree).
- They are **roots** for **unreachable** by default (test-only production code is reachable).
- Flag `--production` removes test roots. Test-only production code then counts as unreachable. Tests remain callers and remain nodes for **unreaching test**.
- **Unreaching test** uses test nodes: the test’s typed call graph does not reach the production function it names.

Framework entries wait for a later spec.
