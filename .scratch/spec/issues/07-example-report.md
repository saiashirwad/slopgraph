Title: Example report
Type: prototype
Status: resolved
Blocked by: 04

## Question

What should a report look like for a small fake program that contains a single-use chain and an empty wrapper?

Build a throwaway example (text or HTML) that a human can read. Link it from this ticket. Use it to confirm What a finding contains before the spec freezes the report.

Prototype (throwaway): [example-report.html](../prototype/example-report.html)

Three layouts of the same fake `src/orders.ts` (one single-use chain, one empty wrapper):

- A — Compact lines (`?variant=A`)
- B — Span blocks (`?variant=B`)
- C — File first, then paths (`?variant=C`)

## Answer

Winner: **C — File first, then paths**.

A report is grouped by file. Each finding is a block with the canonical shape name, the subject location, and an ASCII path as evidence. No snippet required. No remedy.

A is too flat for a graph finding. B’s snippets help empty wrapper, but C’s `return only` on the path is enough. Prototype stays throwaway: [example-report.html](../prototype/example-report.html).
