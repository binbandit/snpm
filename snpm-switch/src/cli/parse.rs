use super::SwitchOptions;

pub(crate) fn parse_switch_options(
    args: Vec<String>,
) -> anyhow::Result<(SwitchOptions, Vec<String>)> {
    let mut options = SwitchOptions::default();
    let mut remaining = Vec::new();
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];

        if arg == "--" {
            remaining.extend(args[index..].iter().cloned());
            break;
        }

        if arg == "--switch-ignore-package-manager" {
            set_ignore_package_manager(&mut options)?;
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--switch-version=") {
            set_version_override(&mut options, value)?;
            index += 1;
            continue;
        }

        if arg == "--switch-version" {
            let value = args
                .get(index + 1)
                .ok_or_else(|| anyhow::anyhow!("--switch-version requires a version argument"))?;
            set_version_override(&mut options, value)?;
            index += 2;
            continue;
        }

        remaining.push(arg.clone());
        index += 1;
    }

    Ok((options, remaining))
}

pub(crate) fn is_meta_command(args: &[String]) -> bool {
    matches!(
        args.first().map(|arg| arg.as_str()),
        Some("--version" | "-V" | "--help" | "-h")
    )
}

fn set_ignore_package_manager(options: &mut SwitchOptions) -> anyhow::Result<()> {
    if options.version_override.is_some() {
        anyhow::bail!("Cannot combine --switch-ignore-package-manager with --switch-version");
    }

    options.ignore_package_manager = true;
    Ok(())
}

fn set_version_override(options: &mut SwitchOptions, value: &str) -> anyhow::Result<()> {
    if options.ignore_package_manager {
        anyhow::bail!("Cannot combine --switch-version with --switch-ignore-package-manager");
    }

    if value.is_empty() {
        anyhow::bail!("--switch-version requires a non-empty version");
    }

    match options.version_override.as_deref() {
        Some(existing) if existing == value => Ok(()),
        Some(existing) => anyhow::bail!(
            "Conflicting --switch-version values: '{}' and '{}'",
            existing,
            value
        ),
        None => {
            options.version_override = Some(value.to_string());
            Ok(())
        }
    }
}
