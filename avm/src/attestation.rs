use {
    crate::HTTP_CLIENT,
    anyhow::{bail, Context, Result},
    reqwest::{header::USER_AGENT, StatusCode},
    semver::Version,
    serde::Deserialize,
    serde_json::Value,
    sha2::{Digest, Sha256},
    sigstore_verify::{
        trust_root::{TrustedRoot, SIGSTORE_PRODUCTION_TRUSTED_ROOT},
        types::{Bundle, Sha256Hash, SignatureContent, Statement},
        VerificationPolicy, Verifier,
    },
    std::{fs::File, io::Read, path::Path},
};

const GITHUB_REPOSITORY: &str = "otter-sec/anchor";
const GITHUB_ACTIONS_ISSUER: &str = "https://token.actions.githubusercontent.com";
const SLSA_PROVENANCE_V1: &str = "https://slsa.dev/provenance/v1";
const RELEASE_WORKFLOW: &str = ".github/workflows/build-cli.yaml";
const NIGHTLY_WORKFLOW: &str = ".github/workflows/nightly-attested-binaries.yaml";
const NIGHTLY_REF: &str = "refs/heads/master";
const FIRST_ATTESTED_RELEASE: Version = Version::new(1, 1, 0);

#[derive(Deserialize)]
struct AttestationsResponse {
    attestations: Vec<GitHubAttestation>,
}

#[derive(Deserialize)]
struct GitHubAttestation {
    bundle: Value,
}

/// What the nightly manifest claims about an artifact, checked against its attestation.
pub(crate) struct NightlyProvenance<'a> {
    /// Full commit hash the build must come from.
    pub source_commit: &'a str,
    /// File name the build workflow attested the archive under.
    pub subject_name: &'a str,
}

pub(crate) fn verify_release(path: &Path, version: &Version) -> Result<()> {
    if !release_has_attestation(version) {
        eprintln!("Anchor v{version} does not have signed releases, skipping checks");
        return Ok(());
    }
    verify(path, &release_identity(version), None)
}

pub(crate) fn verify_nightly(path: &Path, expected: &NightlyProvenance) -> Result<()> {
    verify(path, &nightly_identity(), Some(expected))
}

fn release_identity(version: &Version) -> String {
    workflow_identity(RELEASE_WORKFLOW, &format!("refs/tags/v{version}"))
}

fn release_has_attestation(version: &Version) -> bool {
    // 1.0.3 was backported after attestation was introduced
    version >= &FIRST_ATTESTED_RELEASE || version == &Version::new(1, 0, 3)
}

fn nightly_identity() -> String {
    workflow_identity(NIGHTLY_WORKFLOW, NIGHTLY_REF)
}

fn nightly_source_uri() -> String {
    format!("git+https://github.com/{GITHUB_REPOSITORY}@{NIGHTLY_REF}")
}

fn workflow_identity(workflow: &str, git_ref: &str) -> String {
    format!("https://github.com/{GITHUB_REPOSITORY}/{workflow}@{git_ref}")
}

fn verify(path: &Path, expected_identity: &str, nightly: Option<&NightlyProvenance>) -> Result<()> {
    let digest = sha256_file(path)?;
    let attestations = fetch_attestations(digest).with_context(|| {
        format!(
            "Fetching build provenance attestations for `{}`",
            path.display()
        )
    })?;
    verify_attestations(&attestations, digest, expected_identity, nightly)
}

fn fetch_attestations(digest: Sha256Hash) -> Result<Vec<GitHubAttestation>> {
    let url = format!(
        "https://api.github.com/repos/{GITHUB_REPOSITORY}/attestations/sha256:{}",
        digest.to_hex()
    );
    let response = HTTP_CLIENT
        .get(&url)
        .header(USER_AGENT, "avm https://github.com/otter-sec/anchor")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .query(&[("predicate_type", SLSA_PROVENANCE_V1)])
        .send()
        .with_context(|| format!("Fetching attestations from {url}"))?;

    if response.status() == StatusCode::NOT_FOUND {
        bail!(
            "No build provenance attestation found for digest `sha256:{}`",
            digest.to_hex()
        );
    }
    if !response.status().is_success() {
        bail!(
            "Failed to fetch build provenance attestations for digest `sha256:{}` (status {})",
            digest.to_hex(),
            response.status()
        );
    }

    let response = response
        .json::<AttestationsResponse>()
        .context("Parsing GitHub attestation response")?;
    Ok(response.attestations)
}

fn verify_attestations(
    attestations: &[GitHubAttestation],
    digest: Sha256Hash,
    expected_identity: &str,
    nightly: Option<&NightlyProvenance>,
) -> Result<()> {
    if attestations.is_empty() {
        bail!(
            "No build provenance attestation found for digest `sha256:{}`",
            digest.to_hex()
        );
    }

    let trusted_root = TrustedRoot::from_json(SIGSTORE_PRODUCTION_TRUSTED_ROOT)
        .context("Loading Sigstore production trust root")?;
    let verifier = Verifier::new(&trusted_root);
    let policy = VerificationPolicy::default()
        .require_identity(expected_identity)
        .require_issuer(GITHUB_ACTIONS_ISSUER);
    let mut verification_errors = Vec::new();

    for attestation in attestations {
        let result = (|| -> Result<()> {
            let json = serde_json::to_string(&attestation.bundle)
                .context("Serializing Sigstore bundle")?;
            let bundle = Bundle::from_json(&json).context("Parsing Sigstore bundle")?;
            let statement = require_slsa_provenance(&bundle)?;
            verifier
                .verify(digest, &bundle, &policy)
                .context("Verifying Sigstore bundle")?;
            // The statement is only trusted once the bundle signature is verified.
            if let Some(expected) = nightly {
                require_nightly_provenance(&statement, digest, expected)?;
            }
            Ok(())
        })();

        match result {
            Ok(()) => return Ok(()),
            Err(err) => verification_errors.push(err),
        }
    }

    let detail = verification_errors
        .first()
        .map(ToString::to_string)
        .unwrap_or_else(|| "unknown verification failure".to_string());
    bail!(
        "No valid build provenance attestation from `{expected_identity}` for digest `sha256:{}`: \
         {detail}",
        digest.to_hex()
    )
}

fn require_slsa_provenance(bundle: &Bundle) -> Result<Statement> {
    let SignatureContent::DsseEnvelope(envelope) = &bundle.content else {
        bail!("Build provenance attestation is not a DSSE envelope");
    };
    if envelope.payload_type != "application/vnd.in-toto+json" {
        bail!(
            "Build provenance attestation has unexpected payload type `{}`",
            envelope.payload_type
        );
    }

    let statement = serde_json::from_slice::<Statement>(&envelope.decode_payload())
        .context("Parsing in-toto attestation statement")?;
    if statement.predicate_type != SLSA_PROVENANCE_V1 {
        bail!(
            "Build provenance attestation has unexpected predicate type `{}`",
            statement.predicate_type
        );
    }
    Ok(statement)
}

/// Checks that the attestation was made for the commit and file the nightly manifest names, so a
/// manifest cannot pass off an older nightly build, or another tool or target, as the one it lists.
fn require_nightly_provenance(
    statement: &Statement,
    digest: Sha256Hash,
    expected: &NightlyProvenance,
) -> Result<()> {
    let source_uri = nightly_source_uri();
    let source_commits = statement
        .predicate
        .pointer("/buildDefinition/resolvedDependencies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|dependency| {
            dependency.get("uri").and_then(Value::as_str) == Some(source_uri.as_str())
        })
        .filter_map(|dependency| dependency.pointer("/digest/gitCommit")?.as_str())
        .collect::<Vec<_>>();
    if source_commits.is_empty() {
        bail!("Build provenance attestation does not name a source commit from `{source_uri}`");
    }
    if !source_commits.contains(&expected.source_commit) {
        bail!(
            "Build provenance attestation is for commit `{}`, but the nightly manifest is for \
             commit `{}`",
            source_commits.join("`, `"),
            expected.source_commit
        );
    }

    let digest = digest.to_hex();
    let has_subject = statement.subject.iter().any(|subject| {
        subject.name == expected.subject_name
            && subject.digest.sha256.as_deref() == Some(digest.as_str())
    });
    if !has_subject {
        bail!(
            "Build provenance attestation does not list `{}` with digest `sha256:{digest}`",
            expected.subject_name
        );
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<Sha256Hash> {
    let mut file = File::open(path)
        .with_context(|| format!("Opening `{}` for attestation verification", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("Reading `{}`", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(Sha256Hash::from_bytes(hasher.finalize().into()))
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    const COMMIT: &str = "85c5293e7a01aa43b9cf015986bd7c7188ed4a74";
    const ANCHOR_SUBJECT: &str = "anchor-nightly-20261028-85c5293-aarch64-apple-darwin.tar.gz";
    const ANCHOR_DIGEST: &str = "6aa1916e9e4a6aa6852b4d73d53fb5baef486f39f56d1188ba7e6508d7028f24";
    const AVM_SUBJECT: &str = "avm-nightly-20261028-85c5293-aarch64-apple-darwin.tar.gz";
    const AVM_DIGEST: &str = "5920af41445f2b9fd27489ac1a60b211239636387295e76cc5ec49f4dcbbe9b0";

    /// Statement shaped like the one the nightly workflow attests, with both archives of a target.
    fn nightly_statement(predicate: Value) -> Statement {
        serde_json::from_value(json!({
            "_type": "https://in-toto.io/Statement/v1",
            "subject": [
                { "name": ANCHOR_SUBJECT, "digest": { "sha256": ANCHOR_DIGEST } },
                { "name": AVM_SUBJECT, "digest": { "sha256": AVM_DIGEST } },
            ],
            "predicateType": SLSA_PROVENANCE_V1,
            "predicate": predicate,
        }))
        .unwrap()
    }

    fn built_from(uri: &str, commit: &str) -> Value {
        json!({
            "buildDefinition": {
                "resolvedDependencies": [{ "uri": uri, "digest": { "gitCommit": commit } }]
            }
        })
    }

    fn check_nightly(
        statement: &Statement,
        digest: &str,
        source_commit: &str,
        subject_name: &str,
    ) -> Result<()> {
        require_nightly_provenance(
            statement,
            Sha256Hash::from_hex(digest).unwrap(),
            &NightlyProvenance {
                source_commit,
                subject_name,
            },
        )
    }

    #[test]
    fn nightly_provenance_accepts_matching_commit_and_subject() {
        let statement = nightly_statement(built_from(&nightly_source_uri(), COMMIT));
        check_nightly(&statement, ANCHOR_DIGEST, COMMIT, ANCHOR_SUBJECT).unwrap();
        check_nightly(&statement, AVM_DIGEST, COMMIT, AVM_SUBJECT).unwrap();
    }

    #[test]
    fn nightly_provenance_rejects_other_commit() {
        let other = "6b0175552f1ca5ef3055be7f517ca589cd96485f";
        let statement = nightly_statement(built_from(&nightly_source_uri(), other));
        let err = check_nightly(&statement, ANCHOR_DIGEST, COMMIT, ANCHOR_SUBJECT)
            .unwrap_err()
            .to_string();
        assert!(err.contains(other), "{err}");
        assert!(err.contains(COMMIT), "{err}");
    }

    #[test]
    fn nightly_provenance_rejects_commit_from_other_source() {
        for uri in [
            "git+https://github.com/attacker/anchor@refs/heads/master",
            "git+https://github.com/otter-sec/anchor@refs/heads/feature",
        ] {
            let statement = nightly_statement(built_from(uri, COMMIT));
            assert!(
                check_nightly(&statement, ANCHOR_DIGEST, COMMIT, ANCHOR_SUBJECT).is_err(),
                "accepted `{uri}`"
            );
        }
    }

    #[test]
    fn nightly_provenance_rejects_subject_of_other_build() {
        let statement = nightly_statement(built_from(&nightly_source_uri(), COMMIT));
        assert!(check_nightly(
            &statement,
            ANCHOR_DIGEST,
            COMMIT,
            "anchor-nightly-20261122-0123456-aarch64-apple-darwin.tar.gz"
        )
        .is_err());
    }

    #[test]
    fn nightly_provenance_rejects_subject_of_other_tool() {
        let statement = nightly_statement(built_from(&nightly_source_uri(), COMMIT));
        assert!(check_nightly(&statement, ANCHOR_DIGEST, COMMIT, AVM_SUBJECT).is_err());
    }

    #[test]
    fn nightly_provenance_rejects_subject_with_other_digest() {
        // The `avm` archive of the same build offered as `anchor`.
        let statement = nightly_statement(built_from(&nightly_source_uri(), COMMIT));
        assert!(check_nightly(&statement, AVM_DIGEST, COMMIT, ANCHOR_SUBJECT).is_err());
    }

    #[test]
    fn nightly_provenance_rejects_missing_resolved_dependencies() {
        for predicate in [json!({}), json!({ "buildDefinition": {} }), json!(null)] {
            let statement = nightly_statement(predicate);
            assert!(check_nightly(&statement, ANCHOR_DIGEST, COMMIT, ANCHOR_SUBJECT).is_err());
        }
    }

    #[test]
    fn release_identity_is_scoped_to_tagged_build_workflow() {
        assert_eq!(
            release_identity(&Version::new(1, 2, 3)),
            "https://github.com/otter-sec/anchor/.github/workflows/build-cli.yaml@refs/tags/v1.2.3"
        );
    }

    #[test]
    fn nightly_identity_is_scoped_to_master_workflow() {
        assert_eq!(
            nightly_identity(),
            "https://github.com/otter-sec/anchor/.github/workflows/nightly-attested-binaries.yaml@refs/heads/master"
        );
    }

    #[test]
    fn sha256_file_hashes_file_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("artifact");
        std::fs::write(&path, b"anchor").unwrap();

        assert_eq!(
            sha256_file(&path).unwrap().to_hex(),
            "79bfb0e2ba76b9d447606ddbcc494834f05a4c11deb052e74b49ea307a3c5bcd"
        );
    }

    #[test]
    fn fetches_and_verifies_attested_nightly_digest_and_rejects_mismatches() {
        let digest = Sha256Hash::from_hex(
            "7f0f0ad9dcce712f1db299255f374689d3fb30ac6381f8cbbc538361fac52001",
        )
        .unwrap();
        let attestations = fetch_attestations(digest).unwrap();
        let provenance = NightlyProvenance {
            source_commit: "6b0175552f1ca5ef3055be7f517ca589cd96485f",
            subject_name: "anchor-nightly-20260714-6b01755-x86_64-unknown-linux-gnu.tar.gz",
        };

        verify_attestations(
            &attestations,
            digest,
            &nightly_identity(),
            Some(&provenance),
        )
        .unwrap();

        let wrong_digest = Sha256Hash::from_bytes([0; 32]);
        assert!(verify_attestations(
            &attestations,
            wrong_digest,
            &nightly_identity(),
            Some(&provenance)
        )
        .is_err());
        assert!(verify_attestations(
            &attestations,
            digest,
            &release_identity(&Version::new(1, 1, 2)),
            None
        )
        .is_err());

        // This archive is from the 2026-07-14 nightly, so a manifest for a later nightly must not
        // be able to install it.
        let later_build = NightlyProvenance {
            source_commit: "85c5293e7a01aa43b9cf015986bd7c7188ed4a74",
            subject_name: "anchor-nightly-20261028-85c5293-x86_64-unknown-linux-gnu.tar.gz",
        };
        assert!(verify_attestations(
            &attestations,
            digest,
            &nightly_identity(),
            Some(&later_build)
        )
        .is_err());
        let other_tool = NightlyProvenance {
            subject_name: "avm-nightly-20260714-6b01755-x86_64-unknown-linux-gnu.tar.gz",
            ..provenance
        };
        assert!(verify_attestations(
            &attestations,
            digest,
            &nightly_identity(),
            Some(&other_tool)
        )
        .is_err());
    }
}
