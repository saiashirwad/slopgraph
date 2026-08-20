use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use oxc_resolver::{
    ResolveOptions, Resolver, TsconfigDiscovery, TsconfigOptions, TsconfigReferences,
};

use crate::error::Error;
use crate::parse::{ImportedName, ParsedExport, ParsedModule};
use crate::program::{display_path, is_js_file, is_program_file, is_test_file, Program};

pub struct ModuleGraph {
    pub root: PathBuf,
    pub modules: HashMap<PathBuf, Module>,
    /// (exporter abs path, export name) -> importer abs paths
    pub consumers: HashMap<(PathBuf, String), Vec<PathBuf>>,
    /// File-level dependencies: importer abs path -> imported abs paths
    pub file_dependencies: HashMap<PathBuf, Vec<PathBuf>>,
    /// Discovered entry points (package.json fields + root/src index/main/cli files)
    pub entry_points: HashSet<PathBuf>,
    /// Files identified as tests by the test-file predicate
    pub test_files: HashSet<PathBuf>,
}

pub struct Module {
    pub abs: PathBuf,
    pub display: PathBuf,
    pub exports: Vec<ParsedExport>,
}

impl ModuleGraph {
    pub fn build(program: &Program, parsed: Vec<ParsedModule>) -> Result<Self, Error> {
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

        let in_program: HashSet<PathBuf> = program.files.iter().cloned().collect();

        let mut modules = HashMap::new();
        let mut pending_star: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut consumers: HashMap<(PathBuf, String), Vec<PathBuf>> = HashMap::new();
        let mut file_dependencies: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        let mut test_files = HashSet::new();

        for file in &program.files {
            if is_test_file(&program.root, file) {
                test_files.insert(file.clone());
            }
        }

        let entry_points = crate::entry::discover(program);

        for module in &parsed {
            let display = display_path(&program.root, &module.abs);
            modules.insert(
                module.abs.clone(),
                Module {
                    abs: module.abs.clone(),
                    display,
                    exports: module.exports.clone(),
                },
            );
        }

        for module in &parsed {
            for import in &module.imports {
                let resolved = resolve_import(&resolver, &module.abs, &import.specifier);
                let Some(target) = resolved else {
                    continue;
                };
                if is_js_file(&target) {
                    continue;
                }
                if !is_program_file(&target) {
                    continue;
                }
                let Some(target) = in_program
                    .get(&target)
                    .cloned()
                    .or_else(|| in_program.iter().find(|p| paths_equal(p, &target)).cloned())
                else {
                    continue;
                };

                file_dependencies
                    .entry(module.abs.clone())
                    .or_default()
                    .push(target.clone());

                apply_import(
                    &module.abs,
                    &target,
                    &import.names,
                    &modules,
                    &mut consumers,
                    &mut pending_star,
                );
            }
        }

        for (importer, target) in pending_star {
            file_dependencies
                .entry(importer.clone())
                .or_default()
                .push(target.clone());

            let reexports: Vec<ParsedExport> = match modules.get(&target) {
                Some(source) => source
                    .exports
                    .iter()
                    .filter(|e| e.name != "default")
                    .cloned()
                    .collect(),
                None => continue,
            };
            for export in &reexports {
                consumers
                    .entry((target.clone(), export.name.clone()))
                    .or_default()
                    .push(importer.clone());
            }
            if let Some(dest) = modules.get_mut(&importer) {
                for export in reexports {
                    if dest.exports.iter().any(|e| e.name == export.name) {
                        continue;
                    }
                    dest.exports.push(export);
                }
            }
        }

        for list in consumers.values_mut() {
            list.sort();
            list.dedup();
        }

        for list in file_dependencies.values_mut() {
            list.sort();
            list.dedup();
        }

        Ok(ModuleGraph {
            root: program.root.clone(),
            modules,
            consumers,
            file_dependencies,
            entry_points,
            test_files,
        })
    }

    pub fn consumer_group_dir(&self, importer: &Path) -> PathBuf {
        let display = display_path(&self.root, importer);
        display
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

fn apply_import(
    importer: &Path,
    target: &Path,
    names: &[ImportedName],
    modules: &HashMap<PathBuf, Module>,
    consumers: &mut HashMap<(PathBuf, String), Vec<PathBuf>>,
    pending_star: &mut Vec<(PathBuf, PathBuf)>,
) {
    for name in names {
        match name {
            ImportedName::Named(n) => {
                consumers
                    .entry((target.to_path_buf(), n.clone()))
                    .or_default()
                    .push(importer.to_path_buf());
            }
            ImportedName::Default => {
                consumers
                    .entry((target.to_path_buf(), "default".to_string()))
                    .or_default()
                    .push(importer.to_path_buf());
            }
            ImportedName::Namespace => {
                if let Some(source) = modules.get(target) {
                    for export in &source.exports {
                        consumers
                            .entry((target.to_path_buf(), export.name.clone()))
                            .or_default()
                            .push(importer.to_path_buf());
                    }
                }
            }
            ImportedName::StarReexport => {
                pending_star.push((importer.to_path_buf(), target.to_path_buf()));
            }
        }
    }
}

fn resolve_import(resolver: &Resolver, from_file: &Path, specifier: &str) -> Option<PathBuf> {
    let resolution = resolver.resolve_file(from_file, specifier).ok()?;
    let path = strip_verbatim(resolution.full_path());
    Some(path.canonicalize().map(strip_verbatim).unwrap_or(path))
}

fn strip_verbatim(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    a == b || a.to_string_lossy() == b.to_string_lossy()
}
