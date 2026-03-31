mod flags;
mod parse;

pub(crate) use flags::SwitchOptions;
pub(crate) use parse::{is_meta_command, parse_switch_options};

#[cfg(test)]
mod tests;
