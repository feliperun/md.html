//! Fuzz the pipeline invariant (PRD §16 "build → audit pipeline"): any
//! artifact the SAFE build emits must audit SAFE. A safe build that produces
//! an UNSAFE-auditable artifact means the build and audit pipelines disagree
//! about the policy — a parser differential, not a test to relax.

#![no_main]

use libfuzzer_sys::fuzz_target;
use mdhtml_fuzz::{fonts_dir, runtime_dist, source_dir, themes_dir};

fuzz_target!(|source: &str| {
    if let Ok(artifact) = mdhtml::build::build(
        source,
        source_dir(),
        runtime_dist(),
        themes_dir(),
        fonts_dir(),
    ) {
        let report = mdhtml::audit::audit_artifact(&artifact);
        assert!(
            report.safe,
            "a safe build must always audit SAFE: {}",
            report.render()
        );
    }
});
