mod current;
mod matching;

pub use current::{current_cpu, current_os};
pub use matching::{is_compatible, matches_cpu, matches_os};

#[cfg(test)]
mod tests;
