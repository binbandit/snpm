use snpm_core::operations;

use super::CleanArgs;

pub(super) fn build_options(args: &CleanArgs) -> operations::CleanOptions {
    if args.all {
        return operations::CleanOptions::all();
    }

    let has_explicit_selection = args.packages || args.metadata || args.global;

    if has_explicit_selection {
        operations::CleanOptions {
            packages: args.packages,
            metadata: args.metadata,
            global: args.global,
        }
    } else {
        operations::CleanOptions::default()
    }
}
