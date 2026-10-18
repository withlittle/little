//! Prints, in Markdown, the notices of the syntax definitions little
//! highlights code with, which two-face bundles from bat, for the end of
//! little's third-party notices: see
//! `docs/decisions/0054-third-party-notices.md`.

use std::collections::BTreeMap;

use two_face::acknowledgement::{self, LicenseType};

fn main() {
    // As cargo-about does, each text follows all the definitions under it.
    let mut texts: BTreeMap<(String, &str), Vec<String>> = BTreeMap::new();
    let listing = acknowledgement::listing();
    for license in listing.for_syntaxes() {
        if license.needs_acknowledgement() {
            texts
                .entry((name(license.ty), license.text.trim_end()))
                .or_default()
                .push(license.rel_path.display().to_string());
        }
    }

    // It follows cargo-about's notices, which end with a code block.
    println!();
    println!("## Syntax definitions");
    println!();
    println!("little highlights code with syntax definitions that bat collects, and");
    println!("two-face bundles. Each is listed by the path of its licence in two-face,");
    println!("under that licence, and the licence's text follows; two-face lists them all");
    println!("at <{}>.", acknowledgement::url());
    for ((name, text), paths) in texts {
        println!();
        println!("### {name}");
        println!();
        for path in paths {
            println!("- `{path}`");
        }
        println!();
        println!("````text");
        println!("{text}");
        println!("````");
    }
}

/// The licence's full name, as SPDX gives it, and cargo-about writes it.
fn name(ty: LicenseType) -> String {
    match ty {
        LicenseType::Mit => "MIT License".into(),
        LicenseType::Apache2 => "Apache License 2.0".into(),
        LicenseType::Bsd2Clause => r#"BSD 2-Clause "Simplified" License"#.into(),
        LicenseType::Bsd2ClauseFreeBsd => "BSD 2-Clause FreeBSD License".into(),
        LicenseType::Bsd3Clause => r#"BSD 3-Clause "New" or "Revised" License"#.into(),
        other => format!("{other:?}"),
    }
}
