# How knip, slp, ts-prune, and eslint-plugin-unslop define reachability

Primary sources only (cloned GitHub, official README, knip.dev). Date of clone: local `/tmp/reach-research`.

Repos:

- https://github.com/webpro-nl/knip (docs also at https://knip.dev)
- https://github.com/KeepUni/slp
- https://github.com/nadeesha/ts-prune
- https://github.com/skhoroshavin/eslint-plugin-unslop

Slopgraph shapes (from `CONTEXT.md`): single-use chain, empty wrapper, false sharing, unreachable, near-duplicate, tramp data, type clone, unreaching test.

---

## 1. Knip (`webpro-nl/knip`)

### What counts as an entry point

Official: https://knip.dev/explanations/entry-files

Knip starts from **entry files**, then resolves the module graph.

Default `entry` globs (docs):

- `{index,cli,main}.{js,cjs,mjs,jsx,ts,cts,mts,tsx}`
- `src/{index,cli,main}.{js,cjs,mjs,jsx,ts,cts,mts,tsx}`

Also added as entries:

- `package.json` `main`, `bin`, `exports`
- Parsed `package.json` scripts
- Plugin-added entries (frameworks, test runners, Storybook, bundlers, …)
- Plugin **config files** themselves
- Dynamic `require()` / `import()`, `require.resolve()`, `import.meta.resolve()`
- `new URL('./file.js', import.meta.url)`
- Some `Worker` / `child_process` path patterns
- `module.register('./loader.js')`
- Scripts extracted from CI and source (`execa`, tagged `$` templates, …)

Custom `entry` **replaces** defaults; it is not merged.

Unused files formula (https://knip.dev/guides/configuring-project-files):

```
unused files = project files - (entry files + resolved files)
```

**Unused exports in entry files are skipped by default.** `includeEntryExports: true` reports unused exports in user entry source files, but **not** in plugin entry/config files (e.g. `next.config.js`, SvelteKit `+page.svelte`). Docs: https://knip.dev/reference/configuration (`includeEntryExports`).

Reachability is **file + export reference**, not “call graph from a binary.” A file imported from any entry is used. An export imported anywhere in the analysed set is used.

### How tests are treated

Default mode **includes tests**. Test plugins (Vitest, Jest, Playwright, …) add test files as **entries**. Tests that import production code keep that production code “used.”

**Production mode** (`knip --production`, https://knip.dev/features/production-mode):

- Only `entry`/`project` patterns with `!` suffix
- Only production plugin entries
- Only `package.json` `start` script
- Ignores `@internal` exports
- Excludes tests and Storybook from analysis so production-only-from-tests can show as unused

Docs explicitly say: do **not** exclude tests via `ignore` or negated `project`; plugins re-add them as entries.

### What it already detects vs slopgraph

| Slopgraph shape | Knip |
|---|---|
| Unreachable (file) | Yes: unused files |
| Unreachable (export) | Yes: unused exports / types / nsExports / enum members |
| False sharing | **No.** One importer is enough to mark an export used. No consumer-group count. |
| Empty wrapper | **No.** |
| Near-duplicate | **No** (Knip “duplicates” = same symbol exported twice, not body similarity). Issue types: https://knip.dev/reference/issue-types |

### What it does not detect

- Single-use chain (call-graph 1:1 path)
- Tramp data (pass-through parameters)
- Unreaching test (test that does not call the production function it names)
- Empty wrapper
- False sharing (shared module, one consumer group)

Knip can report “used only in tests” **only in production mode**, as unused production files/exports — not as a named “unreaching test” shape.

---

## 2. slp (`KeepUni/slp`)

Sources: `README.md`, `src/core/entry-points.ts`, `src/core/reachability.ts`, `src/detectors/dead-code.ts`, `src/detectors/empty-wrappers.ts`, `src/core/config.ts`, `src/core/insights.ts`.

### What counts as an entry point

`collectEntryPoints` (`src/core/entry-points.ts`):

- Per-package `package.json`: `main`, `module`, `browser`, `bin`, `exports` (wildcards)
- Universal path regexes: `src/index`, `src/main`, `src/App`, root `index`, `middleware`, `instrumentation`, many `*.config` / `.*rc` tool configs
- Framework conventions when detected: Next.js app/pages, SvelteKit `+page`, Astro pages, Remix routes, Nuxt pages/layouts/plugins, SolidStart, Qwik
- Imports (ES / CJS / dynamic) can seed extra entries
- Template consumers (`.astro` / `.vue` / `.svelte`)
- Workspace package imports
- Registry manifests (`_registry.ts` / `registry.ts` path literals)
- Barrel cascade through re-exports

`computeReachability` (`src/core/reachability.ts`): BFS from those entries **plus template roots**, following static imports, re-export specifiers, and dynamic import targets. File-level graph.

If **no entries** and no template roots: `noEntries: true` and **no** unreachable-file list (avoids flagging everything).

Dead-code detector **skips entire entry-point files** (`dead-code.ts`: `if (entryPoints.has(filePath) continue`). Public API on `main`/`exports` is not reported as dead even if unused inside the repo.

Two tiers (README):

- **dead-code**: exported, zero external imports
- **unused-export**: exported, used only in the same file (`export` is redundant)

### How tests are treated

`isTestFilePath` (`src/core/config.ts`):

- Filename: `.test|.spec|.e2e|.bench|.demo|.example|.story|.stories|.fixture|.fixtures|.mock|.mocks.`
- Setup: vitest/jest/playwright setup files
- Dirs: `__tests__`, `tests`, `test`, `e2e`, benches, playground, generated, `__mocks__`, etc.

Detectors for dead-code, empty-wrappers, duplicates, ai-signatures, code-smells **skip test files**. Tests are not production entries unless they also match entry globs. Production used **only from tests** can still look dead, because tests are skipped as consumers.

### What it already detects vs slopgraph

| Slopgraph shape | slp |
|---|---|
| Unreachable file | Yes: insight `orphaned-file` (file not in reachability set) |
| Unreachable export | Yes: `dead-code` |
| Empty wrapper | Yes: `empty-wrappers` detector. Body is only `return g(args)` with same args; also `const x = g(args); return x;`; `return g(args) as T`; JSX `{...props}` pass-through. Many exemptions (hooks, overrides, generics, Next route handlers, `@deprecated`, …). |
| Near-duplicate | Yes: hash + AST structural fingerprint (identifiers/literals normalized) |
| Type clone | Partial: `same-shape-types` (identical fields, different files). Slopgraph also requires no `extends` link — slp README does not state that extra constraint. |
| False sharing | **No** consumer groups. One import from another file is enough. |
| Single-use chain | **Partial / different.** Insight `over-abstraction-chain`: **3+ empty wrappers** in the **same file** `a -> b -> c -> realThing`. Not a whole-program 1-caller/1-callee path of real functions. |
| Dead wrapper | Insight `dead-wrapper-chain`: empty wrapper whose outer name is also dead. |

### What it does not detect

- Tramp data (no parameter-threading detector; grep of `src/` has no “parameter” detector)
- Unreaching test (tests skipped, not analysed for whether they call the named SUT)
- False sharing
- True single-use chain across files (only empty-wrapper chains of depth ≥ 3 in one file)

---

## 3. ts-prune (`nadeesha/ts-prune`)

Sources: README, `src/analyzer.ts`, `src/runner.ts`.

### What counts as an entry point

**Not a framework/package.json entry model.**

- Project = TypeScript program from `tsconfig.json` (`-p`)
- `files` in tsconfig, if present, become `entrypoints` in `runner.ts` and are treated specially in analysis
- Everything else: export vs import accounting with ts-morph

An export is unused if no other file’s import/re-export references it. README: “zero configuration”; not `packageJson.main` aware (confirmed in GitHub issue discussion on README-related issues). Public package `index` exports can look unused if nothing in the same tsconfig imports them.

### How tests are treated

No default test exclusion.

- `-i, --ignore` filters **report lines** (regex)
- `-s, --skip` excludes matching files when deciding **whether code is used** (README: e.g. `.test.ts` so test-only usage does not keep a production export alive)

Without `-s`, tests in the program count as users.

### What it already detects vs slopgraph

| Shape | ts-prune |
|---|---|
| Unreachable export | Yes (name unused across project) |
| Unused in module | Yes: `(used in module)` — export only used locally; hide with `-u` |
| Unreachable file | No dedicated file reachability (except unused exports implying empty public API) |
| Empty wrapper / false sharing / chains / tramp / unreaching test | **No** |

Known gaps from README: dynamic `import()`, string `require`, framework reflection.

---

## 4. eslint-plugin-unslop (`skhoroshavin/eslint-plugin-unslop`)

Sources: README, `openspec/specs/no-false-sharing/spec.md`, `src/rules/no-false-sharing/index.ts`, `src/rules/no-single-use-constants/index.ts`.

### What counts as an entry point

**Architecture entrypoints**, not program roots.

From `settings.unslop.architecture`:

- Module = directory relative to tsconfig `rootDir`
- Default public files: `entrypoints: ['index.ts']` (override per module)
- `shared: true` marks a module for `no-false-sharing`

`no-false-sharing` only runs on files that **are** those configured entrypoints **and** `shared: true`.

This is “public gate of a folder,” not “binary / route / package.json main.”

### How tests are treated

- `no-whitebox-testing`: recognized tests (`*.test.*`, `*.spec.*`, `*.*-test.*`, `*.*-spec.*`) beside a module must import the **public entrypoint**, not sibling internals
- Tests **do count** as consumers for false-sharing if they import the shared symbol (spec: value and type imports count; no test exclusion in `no-false-sharing` spec)
- Unreaching-test shape: **not** implemented. Whitebox rule is about **import path**, not whether the test calls the named production function

### What it already detects vs slopgraph

| Shape | unslop |
|---|---|
| False sharing | **Yes.** Shared entrypoint symbol must have **≥ 2 directory-level consumer groups**. Internal same-module use collapses to one group and is **not enough**. Zero consumers also reported. Aliases and re-exports resolved via TS program. |
| Unreachable export | Partial: 0 consumer groups on **shared entrypoints only**; plus `no-unused-types` (exported types with zero project consumers — README/PR, later versions) |
| Single-use constants | `no-single-use-constants`: module-scope `const` used ≤ 1 time. **Not** slopgraph single-use **chain**. Function/class/call initializers excluded. |
| Empty wrapper / tramp / unreachable file / unreaching test | **No** |

---

## 5. Overlaps with slopgraph

Already covered by existing tools (different names):

1. **Unreachable file** — Knip unused files; slp `orphaned-file`
2. **Unreachable export** — Knip unused exports; slp `dead-code` / `unused-export`; ts-prune unused exports
3. **Empty wrapper** — slp `empty-wrappers` (local AST pass-through, not graph-wide)
4. **False sharing** — unslop `no-false-sharing` only (needs architecture `shared: true` + entrypoints)
5. **Near-duplicate / type clone** — slp duplicates + `same-shape-types` (not knip/ts-prune/unslop)

Entry-point **philosophy**:

- Knip / slp: auto-detect package + framework **roots**, then import graph
- ts-prune: tsconfig program, no package.json entries
- unslop: configured **module** entry files for architecture rules

Tests as reachability:

- Knip default: tests **keep** production alive; `--production` drops tests
- slp: tests **ignored** as files under detectors (production-only-from-tests looks dead)
- ts-prune: optional `-s` to ignore tests as users
- unslop: tests may **satisfy** sharing counts; whitebox rule is import-boundary only

---

## 6. Gaps slopgraph still needs to fill

These slopgraph shapes have **no equivalent** in the four tools as whole-program graph shapes:

| Shape | Gap |
|---|---|
| **Single-use chain** | slp `over-abstraction-chain` is only **empty wrappers**, **same file**, depth ≥ 3. Need 1-caller/1-callee **paths** of ordinary functions across the program. |
| **Tramp data** | None of the four tools track parameters that are only forwarded. |
| **Unreaching test** | None check that a test’s calls reach the production function it **names**. Knip production mode only drops tests as roots. slp skips tests. unslop whitebox is import-path only. |
| **False sharing** (if slopgraph is whole-program) | Only unslop, and only for **configured** shared modules. Knip/slp/ts-prune treat one importer as “used.” Slopgraph still needs consumer **groups by directory** without requiring ESLint architecture config — unless it copies unslop’s model. |
| **Empty wrapper** (if slopgraph is graph-based) | slp already has a strong AST detector. Value-add is wrapping it in program graph (e.g. wrapper on a reachable path vs dead-only). |
| **Unreachable** | Crowded. Slopgraph must define **entry** like Knip/slp (package + routes) **or** like tsconfig files, and whether tests are roots. Differentiator: function-level path from entry, not only file/export import. |

Recommended differentiators for slopgraph (from these sources, not extra invention):

1. Function-level reachability from explicit entry points (not only unused `export`).
2. Consumer-group false sharing without a separate architecture config (or with a smaller default: “shared/” dirs).
3. Single-use **call** chains, not only empty wrappers.
4. Tramp-data parameters.
5. Tests as first-class: either “only reached from tests” or “test name does not reach SUT.”

---

## Source index

- Knip entry files: https://knip.dev/explanations/entry-files
- Knip production: https://knip.dev/features/production-mode
- Knip unused files: https://knip.dev/guides/configuring-project-files
- Knip issue types: https://knip.dev/reference/issue-types
- slp README + `src/core/entry-points.ts`, `reachability.ts`, `detectors/dead-code.ts`, `detectors/empty-wrappers.ts`, `core/insights.ts`, `core/config.ts`
- ts-prune README + `src/runner.ts`, `src/analyzer.ts`
- unslop README + `openspec/specs/no-false-sharing/spec.md` + rule sources
