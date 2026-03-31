use snpm_core::operations;

use super::super::super::style::paint;

pub(super) fn print_summary(counts: &operations::VulnerabilityCounts, total_packages: usize) {
    let total = counts.total();

    if total == 0 {
        println!(
            "{}",
            paint(
                "32;1",
                &format!("No vulnerabilities found in {} packages!", total_packages),
            ),
        );
        return;
    }

    let noun = if total == 1 {
        "vulnerability"
    } else {
        "vulnerabilities"
    };
    println!(
        "{} {} found in {} packages",
        paint("31;1", &total.to_string()),
        noun,
        total_packages,
    );

    let mut parts = Vec::new();
    push_severity_part(&mut parts, counts.critical, "31;1", "31", "critical");
    push_severity_part(&mut parts, counts.high, "91", "91", "high");
    push_severity_part(&mut parts, counts.moderate, "33", "33", "moderate");
    push_severity_part(&mut parts, counts.low, "32", "32", "low");
    push_severity_part(&mut parts, counts.info, "36", "36", "info");

    if !parts.is_empty() {
        println!("Severity: {}", parts.join(" | "));
    }

    println!();
    println!(
        "{}",
        paint(
            "2",
            "Run `snpm audit --fix` to fix vulnerabilities with available patches.",
        ),
    );
}

fn push_severity_part(
    parts: &mut Vec<String>,
    count: u64,
    count_style: &str,
    label_style: &str,
    label: &str,
) {
    if count == 0 {
        return;
    }

    parts.push(format!(
        "{} {}",
        paint(count_style, &count.to_string()),
        paint(label_style, label),
    ));
}
