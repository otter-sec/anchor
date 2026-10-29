use {super::*, clap::Parser};

#[test]
fn historical_helpers_use_private_files_and_disposable_default_signers() {
    let node = include_str!("historical_fork.js");
    let rust = include_str!("historical_fork.rs.txt");
    assert!(node.contains("#signer = anchor.web3.Keypair.generate()"));
    assert!(node.contains("const signer = keypair ?? this.#signer"));
    assert!(
        !node.contains("fromSecretKey"),
        "never load the workspace signing key implicitly"
    );
    assert!(node.contains("mode: 0o700"));
    assert!(node.contains("mode: 0o600"));
    assert!(node.contains("flag: \"wx\""));
    assert!(rust.contains("options.mode(0o600)"));
    assert!(rust.contains("builder.mode(0o700)"));
    assert!(rust.contains("create_new(true)"));
    assert!(GUIDE.contains("leaked signed transaction bytes"));
}

#[test]
fn helpers_start_historical_surfpool_from_the_suite_plan_and_own_only_their_child() {
    let node = include_str!("historical_fork.js");
    let rust = include_str!("historical_fork.rs.txt");
    for helper in [node, rust] {
        for required in [
            "ANCHOR_HISTORICAL_PLAN",
            "anchor-historical-plan/v1",
            "--after",
            "--studio-port",
            "--surfnet-id",
            "--snapshot",
            "surfnet_getSurfnetInfo",
            "surfnet_timeTravel",
            "FIXTURE_RENT_MISMATCH",
            "PROTOCOL",
        ] {
            assert!(helper.contains(required), "{required}");
        }
        assert!(
            !helper.contains("svm-replay"),
            "helpers launch Surfpool, never the standalone fork"
        );
        assert!(!helper.contains("unsafe"));
    }
    assert!(node.contains("module.exports = { HistoricalFork, HistoricalForkError };"));
    assert_eq!(
        node.matches("\n    this.#assertReady();\n").count(),
        2,
        "rpc() and provider() use the shared lifetime guard"
    );
    assert!(rust.contains("impl Drop for HistoricalFork"));
    // Only crates every Rust scaffold declares (v1 and v2 clients differ in
    // their re-exports).
    assert!(
        !rust.contains("anchor_lang"),
        "the helper never reaches through anchor_client::anchor_lang"
    );
    for import in [
        "use solana_pubkey::Pubkey;",
        "use solana_signer::Signer;",
        "use solana_keypair::Keypair;",
        "use anchor_client::Cluster;",
    ] {
        assert!(rust.contains(import), "{import}");
    }
    assert!(
        include_str!("rust.rs").contains(r#"features = [\"curve25519\"]"#),
        "find_program_address needs curve25519"
    );
    let types = include_str!("historical_fork.d.ts");
    for method in [
        "start(",
        "provider(",
        "rpc(",
        "setAccount(",
        "installProgram(",
        "advance(",
        "close(",
    ] {
        assert!(types.contains(method), "{method}");
    }
}

#[test]
fn historical_helper_rpc_guards_follow_owned_child_lifetime() {
    let node = include_str!("historical_fork.js");
    let rust = include_str!("historical_fork.rs.txt");
    assert!(node.contains("this.#child.exitCode !== null || this.#child.signalCode !== null"));
    assert!(node.contains("child.once(\"exit\", () => {\n      this.#verified = false;"));
    assert!(node.contains("fetch: async (input, init) => {\n        this.#assertReady();"));
    assert!(node.contains("clearTimeout(socket?.reconnect_timer_id)"));
    assert!(node.contains("socket.close(1001)"));
    assert!(node.contains("socket?.setAutoReconnect?.(false)"));
    assert!(node.contains("socket.connect = () => {\n      try {\n        this.#assertReady();"));
    assert!(rust.contains("child: Mutex<Child>"));
    assert!(rust.contains("self.child.lock()"));
    assert!(rust.contains(
        "pub fn rpc(&self, method: &str, params: Value) -> Result<Value> {\n        \
         self.ensure_running()?;"
    ));
    assert!(rust.contains(".try_wait().map_err"));
    assert!(GUIDE.contains("not cryptographic endpoint authentication"));
    assert!(GUIDE.contains("external clients do not inherit"));
}

#[test]
fn historical_scaffolding_is_explicit_and_normal_init_stays_unchanged() {
    for enabled in [false, true] {
        let mut args = vec!["anchor", "init", "demo", "--test-template", "mocha"];
        if enabled {
            args.push("--historical");
        }
        let options = crate::Opts::try_parse_from(args).unwrap();
        let crate::Command::Init {
            historical,
            test_template,
            ..
        } = options.command
        else {
            panic!("wrong command");
        };
        assert_eq!(historical, enabled);
        assert_eq!(test_template, TestTemplate::Mocha);
    }
}

#[test]
fn every_rpc_template_supports_historical_setup_and_in_process_templates_refuse_it() {
    for template in [TestTemplate::Mocha, TestTemplate::Jest, TestTemplate::Rust] {
        validate(&template).unwrap();
    }
    for template in [TestTemplate::Litesvm, TestTemplate::Mollusk] {
        assert!(validate(&template)
            .unwrap_err()
            .to_string()
            .contains("in-process"));
    }
}

#[test]
fn node_matrix_keeps_normal_test_runners_and_chooses_the_boundary_in_code() {
    for javascript in [false, true] {
        for jest in [false, true] {
            for version in [AnchorVersion::V1, AnchorVersion::V2] {
                let generated = node::test("my_program", javascript, jest, version);
                for required in [
                    "HistoricalFork.start({ after: \"413023260\" })",
                    "fork.provider()",
                    "fork.installProgram(program.programId, \"target/deploy/my_program.so\")",
                    "await fork.setAccount(counterAddress)",
                    "await fork.advance(1)",
                    "await fork?.close()",
                    "await program.methods",
                ] {
                    assert!(generated.contains(required), "{required}");
                }
                assert!(generated.contains(if jest { "beforeAll(" } else { "before(" }));
                assert!(generated.contains(if jest { "afterAll(" } else { "after(" }));
                assert_eq!(
                    generated.matches("  }, 60_000);").count(),
                    if jest { 2 } else { 0 },
                    "historical Jest calls and cleanup need bounded RPC timeouts"
                );
                assert!(generated.contains(if javascript {
                    "require(\"./historical\")"
                } else {
                    "from \"./historical\""
                }));
                assert!(
                    !generated.contains("svm-replay") && !generated.contains("Replay.installed")
                );
                assert!(
                    !generated.contains("setProvider"),
                    "fork handles stay explicit, never a global provider"
                );
            }
        }
    }
}
