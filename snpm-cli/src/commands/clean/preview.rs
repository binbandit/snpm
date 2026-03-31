use snpm_core::operations;

pub(super) fn print_preview(
    summary: &operations::CleanSummary,
    options: &operations::CleanOptions,
) {
    println!("The following will be removed:");
    println!();

    if options.packages && summary.packages_count > 0 {
        println!(
            "  Cached packages:    {:>5} {}  ({})",
            summary.packages_count,
            pluralize(summary.packages_count, "package", "packages"),
            operations::format_bytes(summary.packages_size)
        );
    }

    if options.metadata && summary.metadata_count > 0 {
        println!(
            "  Metadata cache:     {:>5} {}  ({})",
            summary.metadata_count,
            pluralize(summary.metadata_count, "entry", "entries"),
            operations::format_bytes(summary.metadata_size)
        );
    }

    if options.global && summary.global_count > 0 {
        println!(
            "  Global installs:    {:>5} {}   ({})",
            summary.global_count,
            pluralize(summary.global_count, "item", "items"),
            operations::format_bytes(summary.global_size)
        );
    }

    println!();
    println!(
        "  Total:              {:>5} {}  ({})",
        summary.total_count(),
        pluralize(summary.total_count(), "item", "items"),
        operations::format_bytes(summary.total_size())
    );
}

pub(super) fn pluralize<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}
