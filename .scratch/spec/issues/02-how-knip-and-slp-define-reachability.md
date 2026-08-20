Title: How knip and slp define reachability
Type: research
Status: resolved

## Question

How do existing TypeScript tools define entry points, unused exports, empty wrappers, and reachability?

Primary sources only: knip, KeepUni/slp, ts-prune, and eslint-plugin-unslop (`no-false-sharing`, `no-single-use-constants`). Read their READMEs and the detector source.

Report:

- What counts as an entry point.
- How they treat tests.
- What they already detect that matches our shapes (false sharing, empty wrapper, unreachable).
- What they do not detect (single-use chain, tramp data, unreaching test).
- Gaps that slopgraph would still need to fill.

## Answer

Findings: [how-knip-and-slp-define-reachability.md](../research/how-knip-and-slp-define-reachability.md)

Knip and slp start from `package.json` and framework/plugin entries, then walk the **import** graph. They skip unused exports on entry files by default. Tests keep production code alive unless production mode (knip) or test files are not consumers (slp).

Overlaps: unreachable files/exports (knip, slp, ts-prune); empty wrapper (slp); false sharing only in unslop, and only for configured shared modules; near-duplicate and type clone (slp).

Gaps: single-use **call** chain, tramp data, unreaching test, and function-level reachability (they track files and exports, not calls).
