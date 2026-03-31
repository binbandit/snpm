use super::{SwitchOptions, is_meta_command, parse_switch_options};

#[test]
fn parse_switch_options_strips_switch_flags_anywhere() {
    let (options, args) = parse_switch_options(vec![
        "install".to_string(),
        "--switch-ignore-package-manager".to_string(),
        "--filter".to_string(),
        "pkg".to_string(),
    ])
    .unwrap();

    assert_eq!(
        options,
        SwitchOptions {
            ignore_package_manager: true,
            version_override: None,
        }
    );
    assert_eq!(
        args,
        vec![
            "install".to_string(),
            "--filter".to_string(),
            "pkg".to_string(),
        ]
    );
}

#[test]
fn parse_switch_options_stops_at_double_dash() {
    let (options, args) = parse_switch_options(vec![
        "install".to_string(),
        "--".to_string(),
        "--switch-ignore-package-manager".to_string(),
    ])
    .unwrap();

    assert_eq!(options, SwitchOptions::default());
    assert_eq!(
        args,
        vec![
            "install".to_string(),
            "--".to_string(),
            "--switch-ignore-package-manager".to_string(),
        ]
    );
}

#[test]
fn parse_switch_options_rejects_conflicting_flags() {
    let error = parse_switch_options(vec![
        "--switch-ignore-package-manager".to_string(),
        "--switch-version".to_string(),
        "2026.3.12".to_string(),
    ])
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("Cannot combine --switch-version with --switch-ignore-package-manager")
    );
}

#[test]
fn is_meta_command_recognizes_version_and_help_flags() {
    assert!(is_meta_command(&["--version".to_string()]));
    assert!(is_meta_command(&["-V".to_string()]));
    assert!(is_meta_command(&["--help".to_string()]));
    assert!(is_meta_command(&["-h".to_string()]));
}

#[test]
fn is_meta_command_rejects_regular_commands() {
    assert!(!is_meta_command(&["install".to_string()]));
    assert!(!is_meta_command(&[
        "install".to_string(),
        "--help".to_string()
    ]));
    assert!(!is_meta_command(&[]));
}
