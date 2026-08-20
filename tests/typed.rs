//! Smoke test: oxc call offsets map onto tsgo `getResolvedSignature`.

use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn oxc_offsets_resolve_signatures_on_tsgo() {
    let counts = slopgraph::resolved_call_counts(fixture("typed-calls"))
        .unwrap_or_else(|e| panic!("resolved_call_counts: {e}"));
    eprintln!(
        "typed-calls: {} calls, {} resolved",
        counts.calls, counts.resolved
    );
    assert_eq!(counts.calls, 2, "wrap's target() and mystery's f(1)");
    assert_eq!(
        counts.resolved, 1,
        "target() resolves; f(1) has no signature declaration"
    );
}
