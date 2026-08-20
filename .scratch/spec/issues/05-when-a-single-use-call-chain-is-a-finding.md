Title: When a single-use call chain is a finding
Type: grilling
Status: resolved
Blocked by: 09

## Question

When is a single-use chain a finding, and when is it not?

In-degree 1 is not enough. Named-step functions can help a reader. The precision policy is: miss some slop rather than emit weak findings.

Decide the predicate: file boundary, export, length, name, tests as callers, and minimum chain length. This predicate becomes the style for the other seven detectors.

## Answer

A **single-use chain** finding is a path of **two or more** functions that each have **in-degree 1** on **typed edges**.

- The head that calls into the path may have many callers. Count only the in-degree-1 nodes.
- The path **may cross files**. No helper-file heuristic. No same-file flag.
- **Exported** functions are **not** chain nodes by default. Flag `--include-exported` allows them.
- No name gate. No body-size gate.
- One in-degree-1 function whose body only forwards is **empty wrapper**, not this shape.
- Tests as callers wait for [What counts as an entry point](06-what-counts-as-an-entry-point.md).

Style for other detectors: typed facts only, no name or path-name heuristics, high-precision default, opt-in flags to loosen.
