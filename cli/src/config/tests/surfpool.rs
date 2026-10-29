use super::*;

#[test]
fn test_surfpool_extra_args_round_trips() {
    let toml = BASE_CONFIG.to_owned()
        + r#"
[surfpool]
extra_args = ["--match-datasource", "--feature", "ptokFjwyJtrwCa9Kgo9xoDS59V4QccBGEaRFnRPnSdP"]
"#;
    let config = Config::from_str(&toml).unwrap();
    let extra_args = config
        .surfpool_config
        .as_ref()
        .and_then(|surfpool| surfpool.extra_args.as_ref())
        .unwrap();

    assert_eq!(
        extra_args,
        &vec![
            "--match-datasource".to_string(),
            "--feature".to_string(),
            "ptokFjwyJtrwCa9Kgo9xoDS59V4QccBGEaRFnRPnSdP".to_string(),
        ]
    );

    let serialized = config.to_string();
    let reparsed = Config::from_str(&serialized).unwrap();
    let reparsed_extra_args = reparsed
        .surfpool_config
        .as_ref()
        .and_then(|surfpool| surfpool.extra_args.as_ref())
        .unwrap();
    assert_eq!(reparsed_extra_args, extra_args);
}

#[test]
fn test_surfpool_extra_args_omitted_or_empty() {
    for (section, expected) in [
        ("[surfpool]\n", None),
        ("[surfpool]\nextra_args = []\n", Some(vec![])),
    ] {
        let config = Config::from_str(&(BASE_CONFIG.to_owned() + section)).unwrap();
        let extra_args = config
            .surfpool_config
            .as_ref()
            .and_then(|surfpool| surfpool.extra_args.clone());
        assert_eq!(extra_args, expected);

        let reparsed = Config::from_str(&config.to_string()).unwrap();
        let reparsed_extra_args = reparsed
            .surfpool_config
            .as_ref()
            .and_then(|surfpool| surfpool.extra_args.clone());
        assert_eq!(reparsed_extra_args, expected);
    }
}
