mod lookup;
mod paths;

use super::types::{WhyOptions, WhyResult};
use crate::{Project, Result};

use lookup::load_why_context;
use paths::build_match;

pub fn why(project: &Project, patterns: &[String], options: WhyOptions) -> Result<WhyResult> {
    let (index, targets) = load_why_context(project, patterns)?;
    let max_depth = options.depth.unwrap_or(usize::MAX);
    let matches = targets
        .into_iter()
        .map(|target| build_match(&target, &index, max_depth))
        .collect();

    Ok(WhyResult { matches })
}

#[cfg(test)]
mod tests;
