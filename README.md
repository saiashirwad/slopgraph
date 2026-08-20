# Slopgraph

Slopgraph is a command-line tool that detects graph-shaped slop in TypeScript programs and outputs an analysis report.

The tool analyzes TypeScript programs (`.ts` and `.tsx` files defined in a `tsconfig.json`). It builds a module dependency graph and a typed call graph to identify structural slop across eight detector shapes.

## Features

- **Whole-Program Analysis**: Resolves module paths and TypeScript configurations using oxc and TypeScript compiler IPC.
- **Typed Call Graphs**: Tracks call edges with resolved type signatures to eliminate false positives from syntactic name matches.
- **Eight Shape Detectors**:
  - **Unreachable**: Detects files and functions with no path from an entry point.
  - **Single-Use Chain**: Detects call paths of two or more functions where each function has exactly one caller.
  - **Empty Wrapper**: Detects functions whose body only forwards to another function.
  - **False Sharing**: Detects exported symbols imported by only one consumer directory group.
  - **Near-Duplicate**: Detects distinct functions with matching AST bodies and normalized token sequences.
  - **Tramp Data**: Detects parameters forwarded through intermediate functions without local use.
  - **Type Clone**: Detects distinct type declarations with identical fields and types without inheritance.
  - **Unreaching Test**: Detects test files that import a production module but make zero typed calls to it.
- **Deterministic Text Reports**: Prints findings grouped by file with ASCII evidence paths.
- **Read-Only**: Emits findings and evidence for human review without altering source code.

## Installation

Build the project from source using Rust:

```bash
cargo build --release
```

The compiled binary is located at `target/release/slopgraph`.

## Usage

Run `slopgraph` by providing the path to a `tsconfig.json` file or to the directory that contains it:

```bash
# Analyze a project directory
slopgraph path/to/project

# Analyze a specific tsconfig.json
slopgraph path/to/tsconfig.json

# Exclude test files as reachability roots (production mode)
slopgraph path/to/project --production

# Allow exported functions in single-use chains
slopgraph path/to/project --include-exported

# Combine flags
slopgraph path/to/project --production --include-exported
```

### CLI Flags

| Flag | Description |
|---|---|
| `<PATH>` | Path to a `tsconfig.json` file or project directory. |
| `--production` | Removes test files as entry point roots. Functions called only by tests are reported as unreachable. |
| `--include-exported` | Allows exported functions to be members of single-use chains. |

## Detector Shapes

### 1. Unreachable
Detects files and functions that have no path from an entry point.
- **Entry points**: `main`, `bin`, and `exports` in `package.json`, plus root and `src/` index, main, or CLI files.
- Under default analysis, test files serve as roots.
- With `--production`, test roots are dropped, and test-only functions are flagged as unreachable.
- If an entire file is unreachable, slopgraph reports the file and suppresses duplicate findings for functions inside that file.

### 2. Single-Use Chain
Detects paths of two or more functions where each function in the path has an in-degree of exactly 1 on typed edges.
- Exported functions are excluded by default to protect public interfaces. Use `--include-exported` to include them.
- If an empty forwarding function is part of a single-use chain, it is reported as part of the chain rather than double-reported as an empty wrapper.

### 3. Empty Wrapper
Detects functions whose body only returns or forwards execution to another function on a typed call edge.
- Findings are suppressed if the function is already reported in a single-use chain.

### 4. False Sharing
Detects exported symbols that have only one consumer directory group.
- Emits a finding when all importing modules belong to a single directory.

### 5. Near-Duplicate
Detects pairs of functions with different names whose bodies match with at least 0.7 confidence.
- Requires function bodies with at least 20 AST nodes.
- Compares structures with an AST-kind sequence hash and a 50-token window hash.

### 6. Tramp Data
Detects parameters passed directly into downstream calls on typed edges without any local read operations.

### 7. Type Clone
Detects distinct type declarations (interfaces or type aliases) that have at least 3 identical field names and type annotations without an `extends` link.

### 8. Unreaching Test
Detects test files that import a production module but make zero typed calls to any function in that module.

## Report Output

Slopgraph prints findings as human-readable text grouped by file and sorted deterministically. Each finding provides:
1. Target file path
2. Shape name
3. Finding subject and line number
4. ASCII evidence path showing call relationships and `←── finding` markers

### Example Report

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
