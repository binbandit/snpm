mod advisory;
mod summary;

use snpm_core::operations;

use advisory::print_advisory;
use summary::print_summary;

pub(crate) fn print_table(
    results: &[operations::AuditResult],
    threshold: Option<operations::Severity>,
) -> bool {
    let mut total_counts = operations::VulnerabilityCounts::default();
    let mut total_packages = 0;
    let mut any_vulnerabilities = false;

    for result in results {
        total_packages += result.total_packages;
        total_counts.merge(&result.counts);

        if let Some(member) = &result.workspace_member {
            println!();
            println!("{}", super::super::style::paint("1", member));
        }

        for advisory in &result.advisories {
            any_vulnerabilities = true;
            print_advisory(advisory);
        }
    }

    println!();
    print_summary(&total_counts, total_packages);

    if let Some(threshold) = threshold {
        total_counts.above_threshold(threshold) > 0
    } else {
        any_vulnerabilities
    }
}
