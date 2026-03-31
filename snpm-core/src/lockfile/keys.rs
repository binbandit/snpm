pub(super) fn package_key(name: &str, version: &str) -> String {
    format!("{name}@{version}")
}

pub(super) fn split_dep_key(key: &str) -> Option<(String, String)> {
    let index = key.rfind('@')?;
    let (name, version_part) = key.split_at(index);
    Some((
        name.to_string(),
        version_part.trim_start_matches('@').to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_dep_key_simple() {
        let result = split_dep_key("lodash@4.17.21");
        assert_eq!(result, Some(("lodash".to_string(), "4.17.21".to_string())));
    }

    #[test]
    fn split_dep_key_scoped() {
        let result = split_dep_key("@types/node@18.0.0");
        assert_eq!(
            result,
            Some(("@types/node".to_string(), "18.0.0".to_string()))
        );
    }

    #[test]
    fn split_dep_key_no_at() {
        assert!(split_dep_key("lodash").is_none());
    }
}
