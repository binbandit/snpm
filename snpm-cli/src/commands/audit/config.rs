use anyhow::Result;
use snpm_core::operations;

use std::collections::HashSet;

use super::AuditArgs;

pub(super) fn build_audit_options(
    args: &AuditArgs,
) -> Result<(operations::AuditOptions, Option<operations::Severity>)> {
    let audit_level = args
        .audit_level
        .as_deref()
        .map(|s| s.parse::<operations::Severity>())
        .transpose()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let options = operations::AuditOptions {
        audit_level,
        production_only: args.prod,
        dev_only: args.dev,
        packages: args.packages.clone(),
        ignore_cves: args.ignore_cves.iter().cloned().collect::<HashSet<_>>(),
        ignore_ghsas: args.ignore_ghsas.iter().cloned().collect::<HashSet<_>>(),
        ignore_unfixable: args.ignore_unfixable,
    };

    Ok((options, audit_level))
}
