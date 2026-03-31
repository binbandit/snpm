use snpm_core::operations;

use super::super::super::style::{paint, print_wrapped, severity_badge, terminal_width};

pub(super) fn print_advisory(advisory: &operations::AuditAdvisory) {
    let width = terminal_width();

    println!();
    println!(
        "{} {}",
        severity_badge(advisory.severity),
        paint("1", &advisory.title),
    );
    println!("  {} {}", paint("2", "Package:"), advisory.module_name);
    println!(
        "  {} {}",
        paint("2", "Vulnerable:"),
        paint("31", &advisory.vulnerable_versions),
    );
    println!(
        "  {} {}",
        paint("2", "Patched:"),
        patched_versions_label(advisory),
    );

    if !advisory.cves.is_empty() {
        println!("  {} {}", paint("2", "CVE:"), advisory.cves.join(", "));
    }

    if !advisory.findings.is_empty() {
        println!("  {}", paint("2", "Paths:"));
        for finding in &advisory.findings {
            for path in &finding.paths {
                let formatted = path.replace('>', " > ");
                print_wrapped(&formatted, width, 4);
            }
        }
    }

    if let Some(url) = advisory_url(advisory) {
        println!("  {} {}", paint("2", "More info:"), paint("36", &url));
    }
}

fn patched_versions_label(advisory: &operations::AuditAdvisory) -> String {
    if advisory.patched_versions.is_empty() || advisory.patched_versions == "<0.0.0" {
        paint("33", "No fix available")
    } else {
        paint("32", &advisory.patched_versions)
    }
}

fn advisory_url(advisory: &operations::AuditAdvisory) -> Option<String> {
    if let Some(url) = &advisory.url {
        return Some(url.clone());
    }

    advisory
        .github_advisory_id
        .as_ref()
        .map(|ghsa| format!("https://github.com/advisories/{}", ghsa))
}
