#[derive(Clone, Debug, PartialEq)]
pub struct RegistryProtocol {
    pub name: String,
}

impl RegistryProtocol {
    pub fn npm() -> Self {
        RegistryProtocol {
            name: "npm".to_string(),
        }
    }

    pub fn git() -> Self {
        RegistryProtocol {
            name: "git".to_string(),
        }
    }

    pub fn jsr() -> Self {
        RegistryProtocol {
            name: "jsr".to_string(),
        }
    }

    pub fn file() -> Self {
        RegistryProtocol {
            name: "file".to_string(),
        }
    }

    pub fn custom(name: &str) -> Self {
        RegistryProtocol {
            name: name.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RegistryProtocol;

    #[test]
    fn registry_protocol_equality() {
        assert_eq!(RegistryProtocol::npm(), RegistryProtocol::npm());
        assert_ne!(RegistryProtocol::npm(), RegistryProtocol::git());
        assert_eq!(
            RegistryProtocol::custom("test"),
            RegistryProtocol::custom("test")
        );
    }
}
