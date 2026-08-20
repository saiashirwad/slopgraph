# Slopgraph

A CLI that detects graph-shaped slop in a TypeScript program and prints a report of findings.

## Language

**Slop**:
Generated code that adds cost but does not add enough value.
_Avoid_: Smell, junk, noise

**Shape**:
A kind of slop that a whole-program graph can show.
_Avoid_: Pattern, rule, smell, heuristic

**Finding**:
One instance of a shape in one program.
_Avoid_: Issue, warning, error, smell, hit

**Detector**:
The procedure that emits findings for one shape.
_Avoid_: Rule, lint, check, analyzer

**Report**:
The list of findings that the CLI prints as human text.
_Avoid_: Score, summary-only, diagnostics, JSON report

**Evidence**:
The proof on a finding that a human can check, specific to the shape.
_Avoid_: Reason, message, why, remedy

**Program**:
The set of files that one `tsconfig.json` includes.
_Avoid_: Repo, codebase, package, project-as-one-file

**Entry point**:
A root of reachability: `package.json` `main` / `bin` / `exports`, or an `index` / `main` / `cli` file at the root or under `src/`.
_Avoid_: Main, start file, framework route

**Consumer group**:
The importers of a symbol, grouped by directory.
_Avoid_: Caller, user, client

**Typed edge**:
A call edge taken from TypeScript `getResolvedSignature`.
_Avoid_: Guessed call, name match

**Syntactic edge**:
A call edge taken from a bound name in the syntax tree, without types.
_Avoid_: Name match, heuristic call

## Shapes

**Single-use chain**:
A path of two or more functions that each have one caller on typed edges.
_Avoid_: Helper chain, pipeline, wrapper chain, empty wrapper

**Empty wrapper**:
A function whose body only forwards to one other function.
_Avoid_: Middle man, pass-through, identity

**False sharing**:
An export with only one consumer group.
_Avoid_: Unused export, dead export, utils dump

**Unreachable**:
A function or file with no path from an entry point.
_Avoid_: Dead code, orphan, unused

**Near-duplicate**:
Two functions whose bodies have the same shape after names and literals are normalized.
_Avoid_: Clone, copy-paste, similar helper

**Tramp data**:
A parameter that a function only passes on and never uses.
_Avoid_: Threaded context, drilled prop, unused parameter

**Type clone**:
Two types with the same fields and different names, and with no `extends` link.
_Avoid_: Duplicate type, isomorphic type, dto copy

**Unreaching test**:
A test whose calls do not reach the production function that it names.
_Avoid_: Fake test, mock test, coverage pad
