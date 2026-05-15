mod current;
mod matching;

pub use current::{current_cpu, current_libc, current_os};
pub use matching::{is_compatible, is_compatible_with_libc, matches_cpu, matches_os};

#[cfg(test)]
mod tests;
