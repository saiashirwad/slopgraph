# Slopgraph

A command-line interface (CLI) tool that detects graph-shaped anti-patterns in TypeScript programs and outputs an analysis report.

The tool analyzes a TypeScript program (`.ts` / `.tsx` files defined by a `tsconfig.json` file), builds a module dependency graph and a typed call graph, and detects eight anti-pattern shapes:
- **UNREACHABLE**
- **SINGLE-USE CHAIN**
- **EMPTY WRAPPER**
- **FALSE SHARING**
- **NEAR-DUPLICATE**
- **TRAMP DATA**
- **TYPE CLONE**
- **UNREACHING TEST**

## Status

All eight detectors are fully implemented, verified, and active in the CLI report engine.

## Usage

```bash
# Analyze a project directory or tsconfig.json
slopgraph path/to/project
slopgraph path/to/tsconfig.json

# Analyze in production mode (remove test roots from reachability analysis)
slopgraph path/to/project --production

# Allow exported functions in single-use chains
slopgraph path/to/project --include-exported

# Combine options
slopgraph path/to/project --production --include-exported
```

### CLI Options

- `<PATH>`: Path to a `tsconfig.json` file, or to the directory that contains it.
- `--production`: Removes test files and test functions as reachability roots. Functions that are called only by tests are reported as unreachable. Test files remain in the module graph so unreaching test detection continues to operate.
- `--include-exported`: Allows exported functions to be members of single-use chains. By default, exported functions are excluded from single-use chains because they form public module boundaries.

## Detector Shapes

### 1. UNREACHABLE
Detects files and functions that have no typed-edge call path from any program entry point (or test root).
- Entry points include `main`, `bin`, and `exports` in `package.json`, plus root and `src/` index, main, or cli files.
- Under default analysis, test files serve as reachability roots.
- When `--production` is enabled, test roots are dropped and test-only functions are flagged as unreachable.
- If an entire file is unreachable, slopgraph reports the file at line 1 and suppresses duplicate findings for functions inside that file.

### 2. SINGLE-USE CHAIN
Detects call chains of two or more functions where each function in the chain has an in-degree of exactly one on typed call edges.
- By default, exported functions are excluded from chains to protect public interfaces.
- Use `--include-exported` to include exported functions in chain analysis.
- When an empty forwarding wrapper is part of a single-use chain, it is reported as part of the chain rather than double-reported as an empty wrapper.

### 3. EMPTY WRAPPER
Detects forwarding functions whose body only returns or executes a call to another function on a resolved typed edge.
- Recognizes statement-form calls and direct expression returns.
- Suppressed when the function is already reported inside a single-use chain.

### 4. FALSE SHARING
Detects exported symbols that have only one consumer directory group.
- Emits a finding when all importing modules belong to a single directory (or when the exporter and importer reside in the same directory).

### 5. NEAR-DUPLICATE
Detects pairs of functions with different names whose AST bodies match with at least 0.7 confidence.
- Requires function bodies with at least 20 AST nodes.
- Computes structural similarity with an AST-kind sequence hash and a 50-token window hash (normalizing identifiers to `$ID` and literals to `$LIT`).

### 6. TRAMP DATA
Detects function parameters that are passed directly into downstream calls on typed edges without any local read operations in the enclosing function.

### 7. TYPE CLONE
Detects distinct type declarations (interfaces or type aliases) that have at least three identical field names and type annotations without an `extends` or inheritance relationship.

### 8. UNREACHING TEST
Detects test files that import a production module but make zero typed calls into any function in that module.
- Identifies dead or obsolete test imports that do not exercise the imported production code.

## Report Format

The report is human-readable text. Findings are grouped by file and sorted deterministically. Each finding card provides:
1. Target file path
2. Shape heading in uppercase
3. Finding subject and line number
4. ASCII evidence path with `←── finding` markers and edge annotations

Example report output:

```text
src/pipeline.ts

SINGLE-USE CHAIN
subject: stepOne  (line 9)
runPipeline   (exported, not in chain)
     │
     ▼
stepOne  ←── finding
     │
     ▼
stepTwo
     │
     ▼
stepThree

src/service.ts

FALSE SHARING
subject: sharedService  (line 1)
src/index.ts
     │  one consumer group
     ▼
sharedService  ←── finding

src/wrapper.ts

EMPTY WRAPPER
subject: emptyWrapper  (line 5)
emptyWrapper  ←── finding
     │  return only
     ▼
targetAction
```

Slopgraph produces diagnostic proof for developers to review. It does not modify source code or apply automated patches.

## Documents

- [`.scratch/spec/spec.md`](.scratch/spec/spec.md) — The Version 1 specification
- [`CONTEXT.md`](CONTEXT.md) — Canonical vocabulary and concepts
- [`PROJECT.md`](PROJECT.md) — Project architecture and milestone tracker

## Issue Tracker

Issues are tracked on GitHub. Milestones M1 through M7 complete the full Version 1 specification.
