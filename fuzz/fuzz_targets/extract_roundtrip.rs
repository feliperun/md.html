//! Fuzz the extraction invariant (PRD §16 "extraction"): for any source the
//! safe build accepts, `extract(build(source))` must return the exact source
//! bytes — mutation-free round-trip. The source is stored verbatim (E-FMT-02
//! makes `</script` breakouts impossible), so any drift is data corruption.

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
        let extracted = mdhtml::extract::extract_source(artifact.as_bytes())
            .unwrap_or_else(|error| panic!("a built artifact must extract: {error:?}"));
        assert_eq!(
            extracted,
            source.as_bytes(),
            "extract(build(source)) must equal the source"
        );
    }
});
