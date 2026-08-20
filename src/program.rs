use std::path::{Path, PathBuf};

use oxc_resolver::{ResolveError, Resolver, TsConfig};
use walkdir::WalkDir;

use crate::error::Error;

/// One TypeScript program: the `.ts` / `.tsx` files a tsconfig includes.
pub struct Program {
    pub root: PathBuf,
    pub tsconfig_path: PathBuf,
    pub files: Vec<PathBuf>,
}

pub fn load(path: &Path) -> Result<Program, Error> {
    let tsconfig_path = locate_tsconfig(path)?;
    let root = tsconfig_path
        .parent()
        .unwrap_or(&tsconfig_path)
        .to_path_buf();
    let root = canonicalize(&root)?;
    let tsconfig_path = canonicalize(&tsconfig_path)?;

    let resolver = Resolver::default();
    let tsconfig = resolver
        .resolve_tsconfig(&tsconfig_path)
        .map_err(resolve_error)?;

    let files = enumerate_files(&root, &tsconfig)?;
    Ok(Program {
        root,
        tsconfig_path,
        files,
    })
}

fn locate_tsconfig(path: &Path) -> Result<PathBuf, Error> {
    let path = if path.is_relative() {
        std::env::current_dir()
            .map_err(|e| Error::io(path, e))?
            .join(path)
    } else {
        path.to_path_buf()
    };

    if path.is_file() {
        return Ok(path);
    }
    if path.is_dir() {
        let candidate = path.join("tsconfig.json");
        if candidate.is_file() {
            return Ok(candidate);
        }
        return Err(Error::NoTsConfig(candidate));
    }
    Err(Error::NoTsConfig(path))
}

fn canonicalize(path: &Path) -> Result<PathBuf, Error> {
    dunce_canonicalize(path).map_err(|e| Error::io(path, e))
}

fn dunce_canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    let canonical = path.canonicalize()?;
    Ok(strip_verbatim(canonical))
}

fn strip_verbatim(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

fn resolve_error(err: ResolveError) -> Error {
    Error::Resolve(err.to_string())
}

fn enumerate_files(root: &Path, tsconfig: &TsConfig) -> Result<Vec<PathBuf>, Error> {
    let mut files = Vec::new();

    if let Some(listed) = &tsconfig.files {
        for file in listed {
            let abs = if file.is_absolute() {
                file.clone()
            } else {
                root.join(file)
            };
            if abs.is_file() && is_program_file(&abs) {
                files.push(canonicalize(&abs)?);
            }
        }
    }

    let includes: Vec<PathBuf> = match &tsconfig.include {
        Some(inc) if !inc.is_empty() => inc.clone(),
        None if tsconfig.files.is_none() => vec![PathBuf::from("**/*")],
        Some(_) | None => Vec::new(),
    };

    let excludes = tsconfig.exclude.clone().unwrap_or_default();

    for include in &includes {
        let walk_root = glob_prefix(root, include);
        if !walk_root.exists() {
            continue;
        }
        let walker = WalkDir::new(&walk_root).into_iter().filter_entry(|entry| {
            let name = entry.file_name();
            name != "node_modules" && name != ".git"
        });
        for entry in walker {
            let entry = entry.map_err(|e| {
                let path = e.path().unwrap_or(root).to_path_buf();
                Error::io(path, std::io::Error::other(e.to_string()))
            })?;
            if !entry.file_type().is_file() {
                continue;
            }
            let abs = canonicalize(entry.path())?;
            if !is_program_file(&abs) {
                continue;
            }
            if excluded(&abs, root, &excludes) {
                continue;
            }
            if !matches_include(&abs, root, include) {
                continue;
            }
            files.push(abs);
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

fn glob_prefix(root: &Path, pattern: &Path) -> PathBuf {
    let raw = pattern.to_string_lossy();
    let prefix = raw.split(['*', '?', '[']).next().unwrap_or("");
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() {
        return root.to_path_buf();
    }
    let path = Path::new(prefix);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn matches_include(abs: &Path, root: &Path, include: &Path) -> bool {
    let prefix = glob_prefix(root, include);
    if prefix == root {
        return true;
    }
    abs == prefix || abs.starts_with(&prefix)
}

fn excluded(abs: &Path, root: &Path, excludes: &[PathBuf]) -> bool {
    for exclude in excludes {
        let prefix = glob_prefix(root, exclude);
        if prefix == root {
            continue;
        }
        if abs == prefix || abs.starts_with(&prefix) {
            return true;
        }
    }
    false
}

pub fn is_program_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.ends_with(".d.ts") || name.ends_with(".d.tsx") {
        return false;
    }
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts") | Some("tsx") | Some("mts") | Some("cts")
    )
}

pub fn is_js_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("js") | Some("jsx") | Some("mjs") | Some("cjs")
    )
}

/// Test-file predicate: determines whether a program file is a test file.
///
/// Reused by unreachable file/function detector and unreaching-test detector.
/// A file is considered a test file if:
/// 1. Its file name matches `*.test.ts`, `*.test.tsx`, `*.test.mts`, `*.test.cts`,
///    `*.spec.ts`, `*.spec.tsx`, `*.spec.mts`, `*.spec.cts`,
///    `test.ts`, `test.tsx`, `spec.ts`, or `spec.tsx`.
/// 2. Any directory component in the relative path from the program root is
///    `__tests__`, `tests`, or `test`.
pub fn is_test_file(root: &Path, file: &Path) -> bool {
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.ends_with(".test.ts")
        || name.ends_with(".test.tsx")
        || name.ends_with(".test.mts")
        || name.ends_with(".test.cts")
        || name.ends_with(".spec.ts")
        || name.ends_with(".spec.tsx")
        || name.ends_with(".spec.mts")
        || name.ends_with(".spec.cts")
        || name == "test.ts"
        || name == "test.tsx"
        || name == "spec.ts"
        || name == "spec.tsx"
    {
        return true;
    }
    let rel = file.strip_prefix(root).unwrap_or(file);
    if let Some(parent) = rel.parent() {
        for comp in parent.components() {
            if let std::path::Component::Normal(os_str) = comp {
                if let Some(s) = os_str.to_str() {
                    if s == "__tests__" || s == "tests" || s == "test" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

pub fn display_path(root: &Path, file: &Path) -> PathBuf {
    file.strip_prefix(root)
        .unwrap_or(file)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
        .into()
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use super::*;

    #[test]
    fn test_file_predicate_identifies_tests() {
        let root = Path::new("/workspace/project");

        // Name-based test files
        assert!(is_test_file(root, Path::new("/workspace/project/src/app.test.ts")));
        assert!(is_test_file(root, Path::new("/workspace/project/src/app.spec.ts")));
        assert!(is_test_file(root, Path::new("/workspace/project/src/app.test.tsx")));
        assert!(is_test_file(root, Path::new("/workspace/project/src/app.spec.tsx")));
        assert!(is_test_file(root, Path::new("/workspace/project/src/app.test.mts")));
        assert!(is_test_file(root, Path::new("/workspace/project/src/app.test.cts")));
        assert!(is_test_file(root, Path::new("/workspace/project/src/test.ts")));
        assert!(is_test_file(root, Path::new("/workspace/project/src/spec.ts")));

        // Directory-based test files
        assert!(is_test_file(root, Path::new("/workspace/project/tests/helper.ts")));
        assert!(is_test_file(root, Path::new("/workspace/project/test/unit.ts")));
        assert!(is_test_file(root, Path::new("/workspace/project/src/__tests__/runner.ts")));

        // Production files (not tests)
        assert!(!is_test_file(root, Path::new("/workspace/project/src/index.ts")));
        assert!(!is_test_file(root, Path::new("/workspace/project/src/main.ts")));
        assert!(!is_test_file(root, Path::new("/workspace/project/src/testing_utils.ts")));
        assert!(!is_test_file(root, Path::new("/workspace/project/src/contest.ts")));
    }
}


