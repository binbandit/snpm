use snpm_core::operations;

use super::style::{paint, severity_badge};

pub(crate) fn render_fix_report(result: &operations::FixResult) {
    if !result.fixed.is_empty() {
        println!();
        println!(
            "{}",
            paint(
                "32;1",
                &format!("Fixed {} vulnerabilities:", result.fixed.len())
            )
        );
        println!();

        for fix in &result.fixed {
            println!(
                "  {} {} {} -> {}",
                severity_badge(fix.severity),
                fix.package,
                paint("2", &fix.from_version),
                paint("32", &fix.to_version),
            );
        }
    }

    if !result.unfixable.is_empty() {
        println!();
        println!(
            "{}",
            paint(
                "33;1",
                &format!(
                    "{} vulnerabilities cannot be fixed automatically:",
                    result.unfixable.len()
                ),
            )
        );
        println!();

        for entry in &result.unfixable {
            println!(
                "  {} {}@{} - {}",
                severity_badge(entry.severity),
                entry.package,
                entry.version,
                paint("2", &entry.reason),
            );
        }
    }

    println!();
}
