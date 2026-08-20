# How to detect near-duplicate functions

Question: How can a tool detect near-duplicate functions in TypeScript with high precision?

Sources (primary only):

- KeepUni/slp source: `src/detectors/duplicates.ts`, `src/utils/tokenize.ts`, `src/utils/similarity.ts`, `src/core/config.ts`, README.
- Schleimer, Wilkerson, Aiken, *Winnowing: Local Algorithms for Document Fingerprinting*, SIGMOD 2003, https://theory.stanford.edu/~aiken/publications/papers/sigmod03.pdf
- oxc: parser only in later TS clone tools. oxc does not define a clone detector in this research set.

## Fingerprint method

Use **both** methods. Do not use one method only.

### 1. Token-window hash (near-identical token sequences)

KeepUni/slp:

- Tokenize the file with the TypeScript scanner.
- Skip whitespace, comments, import declarations, and `export ... from` declarations.
- Hash each contiguous window of `DUPLICATE_MIN_TOKENS` (50) **normalized** tokens.
- Put windows in hash buckets. Compare pairs in the same bucket.
- Extend the match while adjacent tokens have the same `normalized` value.
- Clip the match to enclosing function bodies. Drop the pair if the clipped length is less than 50 tokens.
- Score remaining pairs with `rawIdentityRatio` on **raw** token text (not the normalized form).

This pass finds copy-paste that still has the same token order after name and literal rewrite.

Winnowing (Schleimer et al.) is a related document method: hash k-grams, then keep the minimum hash in each window of size `w = t − k + 1`. That method **guarantees** detection of any shared substring of length at least `t`. slp does **not** implement winnowing. slp hashes **every** window of length 50. That is denser than winnowing. It costs more memory. It does not miss a 50-token match because of fingerprint gaps.

### 2. AST-kind sequence (function-body structure)

KeepUni/slp `collectStructuralFunctionDuplicates`:

- Walk each function-like node with a block body.
- Record `SyntaxKind` of each descendant.
- Skip into nested function-like nodes (record the nested function as one kind, then skip its children).
- Drop bodies with fewer than `STRUCTURAL_MIN_NODES` (20) nodes.
- Hash the kind list with FNV-1a.
- Group functions that share the same hash.
- Report the group only if names differ, not all members already sit in a window-hash issue, and path filters do not apply.

This pass finds AI CRUD copies: same syntax tree, different identifiers.

Exact kind-sequence equality is Type-2 structure match (same tree shape). It is not Type-3 edit distance.

### Pair vs both

- Token hash: high precision for long aligned sequences. Misses copies that change statement order or insert a few statements inside the window.
- AST-kind hash: high precision for whole-function same-shape copies. Misses partial copies inside a large function. Treats two bodies as equal even when operators and callees differ, because those nodes still have kinds (identifiers collapse in the **token** pass, not in the kind list: kinds keep `CallExpression` vs `BinaryExpression`).
- slp runs **window hash first**, then structural, and skips structural groups already covered by window matches.

**Version 1 recommendation: both**, same order as slp.

## Normalization of names and literals

From slp `tokenize.ts` `normalize`:

| Token kind | Normalized value |
| --- | --- |
| Identifier, PrivateIdentifier | `$ID` |
| Numeric, BigInt, String, template parts, regex literal | `$LIT` |
| All other non-trivia tokens | original text (keywords, punctuation, operators) |

Whitespace and comments are dropped. Import and re-export statements are dropped.

This matches Winnowing §2.1 property (1): replace variable names with one placeholder (MOSS used `"V"`). Winnowing §5.2: replace all parameters with one constant and increase k by 1. That is enough. Do not implement Baker p-matching (consistent rename maps) in version 1.

`rawIdentityRatio` then counts how many tokens still have identical **raw** text. Renamed copies get a lower similarity than byte-identical copies. slp uses that ratio for confidence, not as a hard reject.

Structural pass: names and literals are **not** in the fingerprint. Only `SyntaxKind` values are hashed. Function **declaration names** are used only to require `distinctNames.size >= 2` (avoid grouping overloads / same name).

## Thresholds that keep precision high

From slp constants and scoring:

| Gate | Value | Role |
| --- | --- | --- |
| `DUPLICATE_MIN_TOKENS` | 50 | Noise floor. Matches Winnowing: choose k so short idioms do not match. |
| `DUPLICATE_MAX_FILE_LINES` | 5000 | Skip huge files. |
| `STRUCTURAL_MIN_NODES` | 20 | Drop tiny function bodies. |
| Window clip | match must stay inside two function bodies | Avoids header/import/JSX list noise. |
| Same-file overlap | skip if starts differ by < 50, or gap after extend < 50 | Avoids self-overlap. |
| Fat bucket | keep at most 30 entries per hash (`FAT_BUCKET_LIMIT`) | Stops common windows from exploding. |
| Path skip | i18n, publication/template, migrations; same basename across files | High-frequency false copies. |
| Same-file JSX/array siblings | skip if lowest common container is JSX element/fragment or array literal | slp 0.1.1. |
| Structural | exact hash match only (`similarity: 1`) | No fuzzy AST in slp. |
| Structural names | at least two distinct function names | |
| CLI `--min-confidence` | default **0.7** | Hide weak window scores. |

Window confidence (`scoreDuplicate`):

```
score = similarity - 0.15
+ 0.05 if cross-file
- 0.15 if len < 50 * 1.5
clamp to [0, 1]
```

Byte-identical 50-token match: similarity 1 → 0.85 (same file) or 0.90 (cross-file). Short matches drop to ~0.70.

Structural confidence (`scoreStructuralDuplicate`): base **0.7**, +0.05 if group size ≥ 3, small bonus for larger node count, clamp to 1.

Winnowing §5.2 (MOSS, informal): there is a sharp k. Slightly smaller k produces many false positives. False positive hash collisions were not reported in years of MOSS. Use a **large k** (slp: 50 tokens), not a low Jaccard floor.

Do **not** use Jaccard 0.7 as a version-1 function gate. slp does not. Winnowing argues for a hard length threshold, not a soft set overlap.

Cluster pairs with union-find on locations so one copy in N files is one finding.

## Cost on a large program

KeepUni/slp:

- Tokenize: one scan per file. Skip tests, generated files, files > 5000 lines, files with < 50 tokens.
- Window index: `O(T)` hashes for T tokens. Each hash walks 50 token strings (not a rolling Karp–Rabin hash; `hashWindow` rehashes the full window).
- Pair work: only inside buckets with ≥ 2 entries, capped at 30 per bucket. Then linear extend + AST body clip.
- Structural: one AST walk per file, group by hash. **No** all-pairs function compare.

Winnowing §1 and §5.2: all-pairs document compare does not scale. Index fingerprints, then look up. Quadratic work only for pairs that already share fingerprints. slp buckets are that index.

Winnowing density: expected fingerprint fraction `2/(w+1)` if you winnow. slp keeps **all** 50-grams, so index size is about one entry per token (minus 49). Memory is the main cost on a large program.

oxc: similarity-ts (oxc_parser) reports that all-pairs TSED is `n(n−1)/2` and is not practical on large programs (V8 heap in TS; still heavy in Rust). That is **not** the version-1 method.

Profile hooks in slp (`SLP_PROFILE`): times tokenize, window, and structural separately.

## Recommended method for version 1

1. **Token-window hash + AST-kind sequence**, both clipped to function bodies (KeepUni/slp).
2. **Normalize** identifiers to `$ID` and literals to `$LIT`. Keep operators and keywords.
3. **Hard floors**: 50 normalized tokens for windows; 20 AST nodes for structural equality.
4. **Exact** structural hash. Do not add tree-edit distance.
5. **Confidence** as slp: window score from raw identity; structural base 0.7. Default hide below 0.7.
6. **Index by hash**. Cap fat buckets. Union-find clusters.
7. Skip tests, generated files, i18n/template/migration paths, same-basename file pairs, JSX/array siblings.
8. Do not ship winnowing in v1 unless memory of all 50-grams is too large. If that happens, switch the window index to winnowing with `k = 50` and `t` a little above 50 (`w = t − k + 1`), as in Schleimer et al.

This matches the product definition of **near-duplicate**: two function bodies with the same shape after names and literals are normalized.
