# Slopgraph

Slopgraph finds graph-shaped slop in a TypeScript program and prints a report.

Point it at a `tsconfig.json`, or at a directory that contains one. It loads the files that config includes, builds a module graph and a typed call graph, and looks for eight shapes. It does not change your source.

Call edges come from TypeScript’s resolved signatures, not from matching names.

## Install

```bash
cargo build --release
```

The binary lands at `target/release/slopgraph`.

## Use

```bash
slopgraph path/to/tsconfig.json
slopgraph path/to/dir
```

`--production` stops treating test files as entry points. Functions that only tests call then show up as unreachable.

`--include-exported` lets exported functions sit on a single-use chain. By default they stay off it, so public surfaces are left alone.

`--color [auto|always|never]` controls terminal color styling (default: `auto`).

```bash
slopgraph path/to/dir --production
slopgraph path/to/dir --include-exported
slopgraph path/to/dir --color always
slopgraph path/to/dir --production --include-exported
```


## Shapes

**Unreachable.** A file or function with no path from an entry point. Entry points are `package.json` `main` / `bin` / `exports`, plus `index`, `main`, or `cli` at the root or under `src/`. Tests count as roots unless you pass `--production`. If a whole file is unreachable, the report names the file and skips the functions inside it.

**Single-use chain.** Two or more functions in a row where each has exactly one caller. Exported functions stay off the chain unless you pass `--include-exported`. If an empty wrapper sits on a chain, it is reported as part of the chain, not twice.

**Empty wrapper.** A function whose body only forwards to another function. Skipped if that function is already on a single-use chain.

**False sharing.** An export whose importers all live in one directory.

**Near-duplicate.** Two functions with different names whose bodies match (confidence at least 0.7). Bodies need at least 20 AST nodes.

**Tramp data.** A parameter that is only passed into a later call and never read.

**Type clone.** Two interfaces or type aliases with the same fields and types, at least three fields, and no `extends` link.

**Unreaching test.** A test file that imports a production module and never calls anything in it.

## Report

Findings are grouped by file. Each finding names the shape, the subject, the line, an explanatory summary sentence, and an ASCII path you can check. If there is nothing to report, it prints nothing.

```text
src/pipeline.ts

SINGLE-USE CHAIN
subject: stepOne  (line 9)
Chain of 4 functions with exactly one caller per function.
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
Export 'sharedService' is imported only within a single consumer group.
src/index.ts
     │  one consumer group
     ▼
sharedService  ←── finding

src/wrapper.ts

EMPTY WRAPPER
subject: emptyWrapper  (line 5)
Function 'emptyWrapper' only forwards calls to 'targetAction'.
emptyWrapper  ←── finding
     │  return only
     ▼
targetAction
```

