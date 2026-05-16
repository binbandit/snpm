use anyhow::Result;
use clap::Args;
use snpm_core::SnpmConfig;
use snpm_core::node::index::{self, LtsField};

#[derive(Args, Debug)]
pub struct LsRemoteArgs {
    /// Only show LTS releases
    #[arg(long = "lts")]
    pub lts: bool,
    /// Limit results to a Node major version (e.g. 20)
    #[arg(long = "major")]
    pub major: Option<u64>,
    /// Force a refresh from nodejs.org (ignores the cached index)
    #[arg(long = "refresh")]
    pub refresh: bool,
    /// Maximum entries to print (default 30; pass 0 for all)
    #[arg(long = "limit", default_value_t = 30)]
    pub limit: usize,
}

pub async fn run(args: LsRemoteArgs, config: &SnpmConfig) -> Result<()> {
    let releases = index::fetch_index(config, args.refresh).await?;

    let mut printed = 0;
    for release in releases {
        if args.lts && release.lts.codename().is_none() {
            continue;
        }
        if let Some(major) = args.major
            && !matches_major(&release.version, major)
        {
            continue;
        }

        let lts_label = match &release.lts {
            LtsField::Codename(name) => format!(" (lts/{})", name.to_ascii_lowercase()),
            _ => String::new(),
        };
        let npm_label = release
            .npm
            .as_deref()
            .map(|npm| format!(" npm@{}", npm))
            .unwrap_or_default();

        println!("  {}{}{}", release.version, lts_label, npm_label);

        printed += 1;
        if args.limit > 0 && printed >= args.limit {
            break;
        }
    }

    if printed == 0 {
        println!("No matching releases.");
    }

    Ok(())
}

fn matches_major(version_with_v: &str, major: u64) -> bool {
    let stripped = version_with_v.strip_prefix('v').unwrap_or(version_with_v);
    let Some((major_str, _)) = stripped.split_once('.') else {
        return false;
    };
    major_str
        .parse::<u64>()
        .map(|m| m == major)
        .unwrap_or(false)
}
