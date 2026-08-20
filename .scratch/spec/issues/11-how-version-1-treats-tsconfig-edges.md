Title: How version 1 treats tsconfig edges
Type: grilling
Status: resolved

## Question

One program is locked. What do path aliases, `allowJs`, and project references do in version 1?

Decide:

- Path aliases (`paths`, `extends`, `baseUrl`): resolve or ignore?
- `allowJs`: do `.js` / `.jsx` files become findings?
- Project references (`references` in tsconfig): merge, ignore, or refuse?

## Answer

- **Path aliases** — resolve `paths`, `extends`, and `baseUrl` with oxc_resolver. Unresolvable imports drop their edges.
- **allowJs** — `.js` / `.jsx` files are never findings. Calls into `.js` drop their edges. Findings only on `.ts` / `.tsx`.
- **Project references** — ignore `references`. Analyze the root program’s own files only. A referenced project is a separate program (monorepo is out of scope).
