Title: How to detect near-duplicate functions
Type: research
Status: resolved

## Question

How can a tool detect near-duplicate functions in TypeScript with high precision?

Primary sources: KeepUni/slp (window hash and AST-kind fingerprint), Moss/winnowing papers or implementations, and any oxc/ts structural clone detector.

Report:

- Fingerprint method (token hash vs AST-kind sequence vs both).
- Normalization of names and literals.
- Thresholds that keep precision high.
- Cost on a large program.
- A recommended method for version 1.

## Answer

Findings: [how-to-detect-near-duplicate-functions.md](../research/how-to-detect-near-duplicate-functions.md)

Version 1 should copy slp’s two passes: 50-token window hash with identifiers and literals normalized (`$ID` / `$LIT`), then exact AST-kind FNV hash for function bodies with at least 20 nodes. Default confidence 0.7. Do not use all-pairs tree edit distance. Cost is an index, not n².
