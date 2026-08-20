use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use oxc::allocator::Allocator;
use oxc::ast::ast::Statement;
use oxc::parser::Parser;
use oxc::span::SourceType;
use oxc_resolver::{
    ResolveOptions, Resolver, TsconfigDiscovery, TsconfigOptions, TsconfigReferences,
};

use crate::call_graph::CallGraph;
use crate::finding::{Evidence, Finding, Location, PathNode, Shape};
use crate::graph::ModuleGraph;
use crate::parse::line_at;
use crate::program::display_path;
use crate::Options;

/// Detect test files that import a production module but do not call any function in that module.
///
/// A finding is emitted when:
/// 1. A test file (in `modules.test_files`) imports a production module (in `modules.modules` and not in `modules.test_files`).
/// 2. Zero typed call edges from that test file reach any function in that production module.
pub fn detect(
    modules: &ModuleGraph,
    calls: &CallGraph,
    _options: &Options,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    // 1. Build adjacency list of call graph
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for edge in &calls.edges {
        adj.entry(edge.from).or_default().push(edge.to);
    }

    // 2. Map file path to function indices in call graph
    let mut file_to_fns: HashMap<&Path, Vec<usize>> = HashMap::new();
    for (i, func) in calls.functions.iter().enumerate() {
        file_to_fns.entry(func.abs.as_path()).or_default().push(i);
    }

    // 3. Iterate deterministically over test files
    let mut test_files: Vec<_> = modules.test_files.iter().collect();
    test_files.sort();

    for test_file in test_files {
        let Some(deps) = modules.file_dependencies.get(test_file.as_path()) else {
            continue;
        };

        // Filter dependencies to production modules
        let mut prod_deps: Vec<_> = deps
            .iter()
            .filter(|dep| !modules.test_files.contains(*dep) && modules.modules.contains_key(*dep))
            .collect();
        prod_deps.sort();

        if prod_deps.is_empty() {
            continue;
        }

        // BFS from all functions in the test file
        let mut reachable = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(root_fns) = file_to_fns.get(test_file.as_path()) {
            for &fn_idx in root_fns {
                if reachable.insert(fn_idx) {
                    queue.push_back(fn_idx);
                }
            }
        }

        while let Some(curr) = queue.pop_front() {
            if let Some(neighbors) = adj.get(&curr) {
                for &next in neighbors {
                    if reachable.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
        }

        let import_locs = resolve_import_locations(test_file.as_path(), modules);
        let test_display = display_path(&modules.root, test_file.as_path());
        let test_label = test_display.to_string_lossy().into_owned();

        // Check reachability for each imported production module
        for &prod_mod in &prod_deps {
            let has_call = file_to_fns
                .get(prod_mod.as_path())
                .is_some_and(|fns| fns.iter().any(|f| reachable.contains(f)));

            if !has_call {
                let (line, span_start) = import_locs.get(prod_mod).copied().unwrap_or((1, 0));
                let prod_display = display_path(&modules.root, prod_mod);
                let prod_label = prod_display.to_string_lossy().into_owned();

                findings.push(Finding {
                    shape: Shape::UnreachingTest,
                    location: Location {
                        file: test_display.clone(),
                        line,
                        span_start,
                    },
                    subject: prod_label.clone(),
                    evidence: Evidence::Path {
                        nodes: vec![
                            PathNode {
                                label: test_label.clone(),
                                annotation: None,
                                is_subject: false,
                            },
                            PathNode {
                                label: prod_label,
                                annotation: None,
                                is_subject: true,
                            },
                        ],
                    },
                });
            }
        }
    }

    findings.sort_by(|a, b| {
        a.location
            .file
            .cmp(&b.location.file)
            .then(a.location.span_start.cmp(&b.location.span_start))
            .then(a.subject.cmp(&b.subject))
    });

    findings
}

fn resolve_import_locations(
    test_abs: &Path,
    modules: &ModuleGraph,
) -> HashMap<PathBuf, (u32, u32)> {
    let mut map = HashMap::new();
    let Ok(source) = fs::read_to_string(test_abs) else {
        return map;
    };
    let allocator = Allocator::new();
    let source_type = SourceType::from_path(test_abs)
        .unwrap_or_else(|_| SourceType::ts())
        .with_module(true);
    let parsed = Parser::new(&allocator, &source, source_type).parse();

    let tsconfig = modules.root.join("tsconfig.json");
    let resolver = Resolver::new(ResolveOptions {
        tsconfig: if tsconfig.exists() {
            Some(TsconfigDiscovery::Manual(TsconfigOptions {
                config_file: tsconfig,
                references: TsconfigReferences::Disabled,
            }))
        } else {
            None
        },
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

    let deps = modules.file_dependencies.get(test_abs);

    for stmt in &parsed.program.body {
        let (specifier, span_start) = match stmt {
            Statement::ImportDeclaration(decl) => (decl.source.value.as_str(), decl.span.start),
            Statement::ExportFromDeclaration(decl) => (decl.source.value.as_str(), decl.span.start),
            Statement::ExportAllDeclaration(decl) => (decl.source.value.as_str(), decl.span.start),
            _ => continue,
        };

        let line = line_at(&source, span_start);

        if let Ok(resolution) = resolver.resolve_file(test_abs, specifier) {
            let mut resolved_path = strip_verbatim(resolution.full_path());
            if let Ok(canon) = resolved_path.canonicalize() {
                resolved_path = strip_verbatim(canon);
            }
            for mod_path in modules.modules.keys() {
                if paths_equal(mod_path, &resolved_path) {
                    map.entry(mod_path.clone()).or_insert((line, span_start));
                    break;
                }
            }
        }

        if let Some(dep_list) = deps {
            for dep in dep_list {
                if map.contains_key(dep) {
                    continue;
                }
                let dep_str = dep.to_string_lossy();
                let spec_trimmed = specifier.trim_start_matches('.').trim_start_matches('/');
                if dep_str.ends_with(spec_trimmed)
                    || dep.file_stem() == Path::new(specifier).file_stem()
                {
                    map.insert(dep.clone(), (line, span_start));
                }
            }
        }
    }

    map
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
    if a == b {
        return true;
    }
    let sa = a.to_string_lossy().replace('\\', "/");
    let sb = b.to_string_lossy().replace('\\', "/");
    sa == sb || sa.eq_ignore_ascii_case(&sb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call_graph::{FnNode, TypedEdge};
    use crate::graph::Module;

    fn make_test_graph(
        root: &Path,
        modules: Vec<(PathBuf, PathBuf)>,
        test_files: HashSet<PathBuf>,
        deps: HashMap<PathBuf, Vec<PathBuf>>,
    ) -> ModuleGraph {
        let mut mod_map = HashMap::new();
        for (abs, display) in modules {
            mod_map.insert(
                abs.clone(),
                Module {
                    abs,
                    display,
                    exports: Vec::new(),
                },
            );
        }
        ModuleGraph {
            root: root.to_path_buf(),
            modules: mod_map,
            consumers: HashMap::new(),
            file_dependencies: deps,
            entry_points: HashSet::new(),
            test_files,
        }
    }

    #[test]
    fn test_unreaching_test_detected() {
        let root = PathBuf::from("/project");
        let prod_file = root.join("src/service.ts");
        let test_file = root.join("tests/service.test.ts");

        let mut test_files = HashSet::new();
        test_files.insert(test_file.clone());

        let mut deps = HashMap::new();
        deps.insert(test_file.clone(), vec![prod_file.clone()]);

        let modules = make_test_graph(
            &root,
            vec![
                (prod_file.clone(), PathBuf::from("src/service.ts")),
                (test_file.clone(), PathBuf::from("tests/service.test.ts")),
            ],
            test_files,
            deps,
        );

        let functions = vec![
            FnNode {
                abs: prod_file.clone(),
                display: PathBuf::from("src/service.ts"),
                name: "doService".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
            FnNode {
                abs: test_file.clone(),
                display: PathBuf::from("tests/service.test.ts"),
                name: "testService".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
        ];

        // No call edge from test to prod
        let calls = CallGraph {
            functions,
            edges: vec![],
            calls: 0,
            resolved: 0,
        };

        let findings = detect(&modules, &calls, &Options::default());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].shape, Shape::UnreachingTest);
        assert_eq!(findings[0].location.file, PathBuf::from("tests/service.test.ts"));
        assert_eq!(findings[0].subject, "src/service.ts");

        let Evidence::Path { nodes } = &findings[0].evidence;
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].label, "tests/service.test.ts");
        assert!(!nodes[0].is_subject);
        assert_eq!(nodes[1].label, "src/service.ts");
        assert!(nodes[1].is_subject);
    }

    #[test]
    fn test_reached_test_not_emitted() {
        let root = PathBuf::from("/project");
        let prod_file = root.join("src/service.ts");
        let test_file = root.join("tests/service.test.ts");

        let mut test_files = HashSet::new();
        test_files.insert(test_file.clone());

        let mut deps = HashMap::new();
        deps.insert(test_file.clone(), vec![prod_file.clone()]);

        let modules = make_test_graph(
            &root,
            vec![
                (prod_file.clone(), PathBuf::from("src/service.ts")),
                (test_file.clone(), PathBuf::from("tests/service.test.ts")),
            ],
            test_files,
            deps,
        );

        let functions = vec![
            FnNode {
                abs: prod_file.clone(),
                display: PathBuf::from("src/service.ts"),
                name: "doService".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
            FnNode {
                abs: test_file.clone(),
                display: PathBuf::from("tests/service.test.ts"),
                name: "testService".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
        ];

        // Call edge from test (1) to prod (0)
        let calls = CallGraph {
            functions,
            edges: vec![TypedEdge {
                from: 1,
                to: 0,
                call_start: 25,
            }],
            calls: 1,
            resolved: 1,
        };

        let findings = detect(&modules, &calls, &Options::default());
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_test_to_test_import_ignored() {
        let root = PathBuf::from("/project");
        let helper_file = root.join("tests/helper.ts");
        let test_file = root.join("tests/unit.test.ts");

        let mut test_files = HashSet::new();
        test_files.insert(helper_file.clone());
        test_files.insert(test_file.clone());

        let mut deps = HashMap::new();
        deps.insert(test_file.clone(), vec![helper_file.clone()]);

        let modules = make_test_graph(
            &root,
            vec![
                (helper_file.clone(), PathBuf::from("tests/helper.ts")),
                (test_file.clone(), PathBuf::from("tests/unit.test.ts")),
            ],
            test_files,
            deps,
        );

        let functions = vec![
            FnNode {
                abs: helper_file.clone(),
                display: PathBuf::from("tests/helper.ts"),
                name: "setup".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
            FnNode {
                abs: test_file.clone(),
                display: PathBuf::from("tests/unit.test.ts"),
                name: "runTest".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
        ];

        let calls = CallGraph {
            functions,
            edges: vec![],
            calls: 0,
            resolved: 0,
        };

        let findings = detect(&modules, &calls, &Options::default());
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_prod_to_prod_import_ignored() {
        let root = PathBuf::from("/project");
        let repo_file = root.join("src/repo.ts");
        let service_file = root.join("src/service.ts");

        let test_files = HashSet::new();
        let mut deps = HashMap::new();
        deps.insert(service_file.clone(), vec![repo_file.clone()]);

        let modules = make_test_graph(
            &root,
            vec![
                (repo_file.clone(), PathBuf::from("src/repo.ts")),
                (service_file.clone(), PathBuf::from("src/service.ts")),
            ],
            test_files,
            deps,
        );

        let functions = vec![
            FnNode {
                abs: repo_file.clone(),
                display: PathBuf::from("src/repo.ts"),
                name: "query".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
            FnNode {
                abs: service_file.clone(),
                display: PathBuf::from("src/service.ts"),
                name: "action".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
        ];

        let calls = CallGraph {
            functions,
            edges: vec![],
            calls: 0,
            resolved: 0,
        };

        let findings = detect(&modules, &calls, &Options::default());
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_transitive_call_reachability() {
        let root = PathBuf::from("/project");
        let prod_file = root.join("src/service.ts");
        let helper_file = root.join("tests/helper.ts");
        let test_file = root.join("tests/unit.test.ts");

        let mut test_files = HashSet::new();
        test_files.insert(helper_file.clone());
        test_files.insert(test_file.clone());

        let mut deps = HashMap::new();
        deps.insert(test_file.clone(), vec![prod_file.clone(), helper_file.clone()]);
        deps.insert(helper_file.clone(), vec![prod_file.clone()]);

        let modules = make_test_graph(
            &root,
            vec![
                (prod_file.clone(), PathBuf::from("src/service.ts")),
                (helper_file.clone(), PathBuf::from("tests/helper.ts")),
                (test_file.clone(), PathBuf::from("tests/unit.test.ts")),
            ],
            test_files,
            deps,
        );

        // 0: prod_fn, 1: helper_fn, 2: test_fn
        let functions = vec![
            FnNode {
                abs: prod_file.clone(),
                display: PathBuf::from("src/service.ts"),
                name: "prod_fn".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
            FnNode {
                abs: helper_file.clone(),
                display: PathBuf::from("tests/helper.ts"),
                name: "helper_fn".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
            FnNode {
                abs: test_file.clone(),
                display: PathBuf::from("tests/unit.test.ts"),
                name: "test_fn".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
        ];

        // test_fn (2) -> helper_fn (1) -> prod_fn (0)
        let calls = CallGraph {
            functions,
            edges: vec![
                TypedEdge {
                    from: 2,
                    to: 1,
                    call_start: 20,
                },
                TypedEdge {
                    from: 1,
                    to: 0,
                    call_start: 30,
                },
            ],
            calls: 2,
            resolved: 2,
        };

        let findings = detect(&modules, &calls, &Options::default());
        assert_eq!(findings.len(), 0);
    }

    #[test]
    fn test_multiple_imports_one_reached_one_unreached() {
        let root = PathBuf::from("/project");
        let prod_a = root.join("src/a.ts");
        let prod_b = root.join("src/b.ts");
        let test_file = root.join("tests/multi.test.ts");

        let mut test_files = HashSet::new();
        test_files.insert(test_file.clone());

        let mut deps = HashMap::new();
        deps.insert(test_file.clone(), vec![prod_a.clone(), prod_b.clone()]);

        let modules = make_test_graph(
            &root,
            vec![
                (prod_a.clone(), PathBuf::from("src/a.ts")),
                (prod_b.clone(), PathBuf::from("src/b.ts")),
                (test_file.clone(), PathBuf::from("tests/multi.test.ts")),
            ],
            test_files,
            deps,
        );

        let functions = vec![
            FnNode {
                abs: prod_a.clone(),
                display: PathBuf::from("src/a.ts"),
                name: "fnA".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
            FnNode {
                abs: prod_b.clone(),
                display: PathBuf::from("src/b.ts"),
                name: "fnB".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
            FnNode {
                abs: test_file.clone(),
                display: PathBuf::from("tests/multi.test.ts"),
                name: "testFn".to_string(),
                line: 1,
                span_start: 10,
                exported: true,
                forward: None,
            },
        ];

        // testFn (2) -> fnA (0)
        let calls = CallGraph {
            functions,
            edges: vec![TypedEdge {
                from: 2,
                to: 0,
                call_start: 20,
            }],
            calls: 1,
            resolved: 1,
        };

        let findings = detect(&modules, &calls, &Options::default());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].shape, Shape::UnreachingTest);
        assert_eq!(findings[0].subject, "src/b.ts");
    }

    #[test]
    fn test_module_with_no_functions_is_reported_when_imported_by_test() {
        let root = PathBuf::from("/project");
        let types_file = root.join("src/types.ts");
        let test_file = root.join("tests/types.test.ts");

        let mut test_files = HashSet::new();
        test_files.insert(test_file.clone());

        let mut deps = HashMap::new();
        deps.insert(test_file.clone(), vec![types_file.clone()]);

        let modules = make_test_graph(
            &root,
            vec![
                (types_file.clone(), PathBuf::from("src/types.ts")),
                (test_file.clone(), PathBuf::from("tests/types.test.ts")),
            ],
            test_files,
            deps,
        );

        // types_file has 0 functions
        let functions = vec![FnNode {
            abs: test_file.clone(),
            display: PathBuf::from("tests/types.test.ts"),
            name: "testTypes".to_string(),
            line: 1,
            span_start: 10,
            exported: true,
            forward: None,
        }];

        let calls = CallGraph {
            functions,
            edges: vec![],
            calls: 0,
            resolved: 0,
        };

        let findings = detect(&modules, &calls, &Options::default());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].subject, "src/types.ts");
    }

    #[test]
    fn test_multiple_test_files_deterministic_ordering() {
        let root = PathBuf::from("/project");
        let prod_a = root.join("src/a.ts");
        let prod_b = root.join("src/b.ts");
        let test_1 = root.join("tests/1.test.ts");
        let test_2 = root.join("tests/2.test.ts");

        let mut test_files = HashSet::new();
        test_files.insert(test_1.clone());
        test_files.insert(test_2.clone());

        let mut deps = HashMap::new();
        deps.insert(test_1.clone(), vec![prod_a.clone()]);
        deps.insert(test_2.clone(), vec![prod_b.clone()]);

        let modules = make_test_graph(
            &root,
            vec![
                (prod_a.clone(), PathBuf::from("src/a.ts")),
                (prod_b.clone(), PathBuf::from("src/b.ts")),
                (test_1.clone(), PathBuf::from("tests/1.test.ts")),
                (test_2.clone(), PathBuf::from("tests/2.test.ts")),
            ],
            test_files,
            deps,
        );

        let calls = CallGraph {
            functions: vec![],
            edges: vec![],
            calls: 0,
            resolved: 0,
        };

        let findings = detect(&modules, &calls, &Options::default());
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].location.file, PathBuf::from("tests/1.test.ts"));
        assert_eq!(findings[0].subject, "src/a.ts");
        assert_eq!(findings[1].location.file, PathBuf::from("tests/2.test.ts"));
        assert_eq!(findings[1].subject, "src/b.ts");
    }
}
