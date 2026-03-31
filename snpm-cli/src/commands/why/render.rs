use anyhow::Result;
use snpm_core::{console, operations};

pub(super) fn print_no_results(package: &str) {
    console::info(&format!("No dependency paths found for '{}'.", package));
}

pub(super) fn print_result(result: &operations::WhyResult, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(result)?);
        return Ok(());
    }

    for (idx, matched) in result.matches.iter().enumerate() {
        if idx > 0 {
            println!();
        }

        println!("{}@{}", matched.name, matched.version);

        if matched.paths.is_empty() {
            println!("  (no dependents found)");
            continue;
        }

        for (path_idx, path) in matched.paths.iter().enumerate() {
            if path_idx > 0 {
                println!();
            }

            if path.hops.is_empty() {
                println!("  (no parent packages found)");
                continue;
            }

            for (hop_idx, hop) in path.hops.iter().enumerate() {
                let indent = "  ".repeat(hop_idx + 1);

                match hop {
                    operations::WhyHop::Package { name, version, via } => {
                        println!("{}<- {}@{} (via {})", indent, name, version, via);
                    }
                    operations::WhyHop::Root { name, requested } => {
                        println!("{}<- root:{} (requested {})", indent, name, requested);
                    }
                }
            }

            if path.truncated {
                let indent = "  ".repeat(path.hops.len() + 1);
                println!("{}<- ... (max depth reached)", indent);
            }
        }
    }

    Ok(())
}
