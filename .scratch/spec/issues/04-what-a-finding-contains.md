Title: What a finding contains
Type: grilling
Status: resolved

## Question

What fields does one finding have, and what does the CLI print for a report?

Decide:

- Identity of the shape.
- Location (file, span).
- Evidence (for example a call path).
- Whether the finding includes a suggested remedy, or only the shape.
- Human text vs machine text (or both) in version 1.

Use the glossary: finding, shape, report. Do not call a finding an error.

## Answer

Every finding has:

- **shape** — canonical name from `CONTEXT.md`
- **location** — file and span of the subject
- **evidence** — shape-specific proof a human can check (for a single-use chain: the function path)

A finding does **not** include a remedy or a patch.

Version 1 prints a **human-text report** only. No JSON flag. [Example report](07-example-report.md) locks the look.
