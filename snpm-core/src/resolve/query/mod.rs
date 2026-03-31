mod build;
mod parse;
mod request;

pub use build::build_dep_request;
pub use parse::split_protocol_spec;
pub use request::DepRequest;

#[cfg(test)]
mod tests;
