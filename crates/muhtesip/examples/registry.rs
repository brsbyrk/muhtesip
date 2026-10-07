//! Print the rule table the registry publishes, then show an adjustment taking effect.
//!
//! This is the embedding story in miniature: a host program links `muhtesip` and reads rule facts —
//! and re-runs the linter with its own severity policy — in-process, with no subprocess and no
//! parsing of tool output.

/// A small document that trips both shipped rules.
const SAMPLE: &str = "\
name: ci
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: example/toolbox@v4
";

fn main() {
    let registry = muhtesip::Registry::all();

    println!("{:<18} {:<8} DESCRIPTION", "RULE", "SEVERITY");
    for meta in registry.metas() {
        println!("{:<18} {:<8} {}", meta.id, meta.severity, meta.description);
    }

    if let Some(first) = registry.metas().first() {
        println!();
        println!("docs example: {}", first.docs_url());
    }

    println!();
    println!("default policy:");
    report(muhtesip::lint(SAMPLE));

    let policy = muhtesip::RuleSettings::new().with(
        "unpinned-action",
        muhtesip::RuleOverride {
            severity: Some(muhtesip::Severity::Error),
            ..Default::default()
        },
    );
    let configured = muhtesip::Registry::all().configure(policy);

    println!();
    println!("with unpinned-action escalated to error:");
    report(muhtesip::lint_str(SAMPLE, &configured));
}

/// Print one line per finding, or say the document was clean.
fn report(result: Result<Vec<muhtesip::Finding>, muhtesip::Error>) {
    match result {
        Ok(findings) if findings.is_empty() => println!("  clean"),
        Ok(findings) => {
            for finding in findings {
                println!(
                    "  line {}: {}: {} [{}]",
                    finding.line, finding.severity, finding.message, finding.rule
                );
            }
        }
        Err(error) => eprintln!("  could not lint sample: {error}"),
    }
}
