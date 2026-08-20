use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use oxc_resolver::{
    ResolveOptions, Resolver, TsconfigDiscovery, TsconfigOptions, TsconfigReferences,
};
use serde_json::Value;

use crate::program::{is_program_file, is_test_file, Program};

/// Discover entry points for a TypeScript program.
///
/// Entry points come from:
/// 1. `package.json` fields `main`, `bin`, and `exports`.
/// 2. Files named `index`, `main`, or `cli` at the repo root or directly under `src/`
///    with TypeScript extensions (`.ts`, `.tsx`, `.mts`, `.cts`).
///
/// Framework routes are not entry points.
pub fn discover(program: &Program) -> HashSet<PathBuf> {
    let mut entry_points = HashSet::new();

    // 1. Files named index, main, or cli at the repo root or directly under src/
    for file in &program.files {
        if is_test_file(&program.root, file) {
            continue;
        }
        let rel = file.strip_prefix(&program.root).unwrap_or(file);
        let parent = rel.parent().map(|p| p.to_string_lossy()).unwrap_or_default();
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if (parent.is_empty() || parent == "src")
            && (stem == "index" || stem == "main" || stem == "cli")
        {
            entry_points.insert(file.clone());
        }
    }

    // 2. package.json fields main, bin, and exports
    let package_json_path = program.root.join("package.json");
    if package_json_path.is_file() {
        if let Ok(content) = fs::read_to_string(&package_json_path) {
            if let Ok(json) = serde_json::from_str::<Value>(&content) {
                let mut candidates = Vec::new();

                // "main"
                if let Some(Value::String(main)) = json.get("main") {
                    candidates.push(main.clone());
                }

                // "bin"
                match json.get("bin") {
                    Some(Value::String(bin)) => {
                        candidates.push(bin.clone());
                    }
                    Some(Value::Object(map)) => {
                        for val in map.values() {
                            if let Value::String(bin) = val {
                                candidates.push(bin.clone());
                            }
                        }
                    }
                    _ => {}
                }

                // "exports"
                if let Some(exports) = json.get("exports") {
                    collect_export_strings(exports, &mut candidates);
                }

                let resolver = Resolver::new(ResolveOptions {
                    tsconfig: Some(TsconfigDiscovery::Manual(TsconfigOptions {
                        config_file: program.tsconfig_path.clone(),
                        references: TsconfigReferences::Disabled,
                    })),
                    extensions: vec![
                        ".ts".into(),
                        ".tsx".into(),
                        ".d.ts".into(),
                        ".js".into(),
                        ".jsx".into(),
                        ".mjs".into(),
                        ".cjs".into(),
                        ".json".into(),
                    ],
                    extension_alias: vec![
                        (
                            ".js".into(),
                            vec![".ts".into(), ".tsx".into(), ".js".into(), ".jsx".into()],
                        ),
                        (".jsx".into(), vec![".tsx".into(), ".jsx".into()]),
                        (".cjs".into(), vec![".cts".into(), ".cjs".into()]),
                        (".mjs".into(), vec![".mts".into(), ".mjs".into()]),
                    ],
                    condition_names: vec![
                        "types".into(),
                        "import".into(),
                        "require".into(),
                        "default".into(),
                    ],
                    ..ResolveOptions::default()
                });

                for candidate in candidates {
                    resolve_candidate(&candidate, program, &resolver, &mut entry_points);
                }
            }
        }
    }

    entry_points
}

fn collect_export_strings(val: &Value, out: &mut Vec<String>) {
    match val {
        Value::String(s) => out.push(s.clone()),
        Value::Array(arr) => {
            for item in arr {
                collect_export_strings(item, out);
            }
        }
        Value::Object(map) => {
            for v in map.values() {
                collect_export_strings(v, out);
            }
        }
        _ => {}
    }
}

fn resolve_candidate(
    candidate: &str,
    program: &Program,
    resolver: &Resolver,
    entry_points: &mut HashSet<PathBuf>,
) {
    // 1. Direct path check relative to program root
    let direct = program.root.join(candidate);
    if try_add_file(&direct, program, entry_points) {
        return;
    }

    // 2. Try substituting JS extension with TS extension
    for ext in &[".ts", ".tsx", ".mts", ".cts"] {
        let replaced = replace_js_extension(candidate, ext);
        if try_add_file(&program.root.join(&replaced), program, entry_points) {
            return;
        }
    }

    // 3. Try mapping dist/ or build/ or lib/ or out/ to src/ or root
    for prefix in &[
        "dist/", "build/", "lib/", "out/", "./dist/", "./build/", "./lib/", "./out/",
    ] {
        if let Some(stripped) = candidate.strip_prefix(prefix) {
            let under_src = program.root.join("src").join(stripped);
            if try_add_file(&under_src, program, entry_points) {
                return;
            }
            for ext in &[".ts", ".tsx", ".mts", ".cts"] {
                let replaced = replace_js_extension(stripped, ext);
                let under_src_replaced = program.root.join("src").join(&replaced);
                if try_add_file(&under_src_replaced, program, entry_points) {
                    return;
                }
                let under_root_replaced = program.root.join(&replaced);
                if try_add_file(&under_root_replaced, program, entry_points) {
                    return;
                }
            }
        }
    }

    // 4. Try oxc_resolver
    let specifier = if candidate.starts_with('.') || candidate.starts_with('/') {
        candidate.to_string()
    } else {
        format!("./{}", candidate)
    };

    if let Ok(res) = resolver.resolve(&program.root, &specifier) {
        let path = res.full_path();
        if try_add_file(&path, program, entry_points) {
            return;
        }
        for ext in &[".ts", ".tsx", ".mts", ".cts"] {
            let replaced = path.with_extension(ext.trim_start_matches('.'));
            if try_add_file(&replaced, program, entry_points) {
                return;
            }
        }
    }

}

fn try_add_file(path: &Path, program: &Program, entry_points: &mut HashSet<PathBuf>) -> bool {
    let Ok(canon) = path.canonicalize() else {
        return false;
    };
    if is_program_file(&canon) && program.files.iter().any(|f| paths_equal(f, &canon)) {
        entry_points.insert(canon);
        return true;
    }
    false
}

fn replace_js_extension(path_str: &str, new_ext: &str) -> String {
    for old_ext in &[".js", ".jsx", ".mjs", ".cjs"] {
        if let Some(prefix) = path_str.strip_suffix(old_ext) {
            return format!("{}{}", prefix, new_ext);
        }
    }
    let file_name = path_str.rsplit('/').next().unwrap_or(path_str);
    if !file_name.contains('.') {
        return format!("{}{}", path_str, new_ext);
    }
    path_str.to_string()
}



fn paths_equal(a: &Path, b: &Path) -> bool {
    a == b || a.to_string_lossy() == b.to_string_lossy()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn collects_nested_export_strings() {
        let val = json!({
            ".": {
                "import": "./src/index.ts",
                "require": "./src/index.cjs"
            },
            "./feature": [
                "./src/feature1.ts",
                { "default": "./src/feature2.ts" }
            ]
        });
        let mut out = Vec::new();
        collect_export_strings(&val, &mut out);
        out.sort();
        assert_eq!(
            out,
            vec![
                "./src/feature1.ts",
                "./src/feature2.ts",
                "./src/index.cjs",
                "./src/index.ts"
            ]
        );
    }

    #[test]
    fn replaces_js_extension_correctly() {
        assert_eq!(replace_js_extension("./dist/app.js", ".ts"), "./dist/app.ts");
        assert_eq!(replace_js_extension("./dist/app.mjs", ".mts"), "./dist/app.mts");
        assert_eq!(replace_js_extension("./dist/app.cjs", ".cts"), "./dist/app.cts");
        assert_eq!(replace_js_extension("./src/index", ".ts"), "./src/index.ts");
    }
}

