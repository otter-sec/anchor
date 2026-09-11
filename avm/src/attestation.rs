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
const NIGHTLY_GIT_REF: &str = "refs/heads/master";
const FIRST_ATTESTED_RELEASE: Version = Version::new(1, 1, 0);

#[derive(Deserialize)]
struct AttestationsResponse {
    attestations: Vec<GitHubAttestation>,
}

#[derive(Deserialize)]
struct GitHubAttestation {
    bundle: Value,
}

pub(crate) fn verify_release(path: &Path, version: &Version) -> Result<()> {
    if !release_has_attestation(version) {
        eprintln!("Anchor v{version} does not have signed releases, skipping checks");
        return Ok(());
    }
    verify(path, &release_identity(version), None)
}

pub(crate) fn verify_nightly(path: &Path, source_commit: &str, subject: &str) -> Result<()> {
    verify(path, &nightly_identity(), Some((source_commit, subject)))
}

fn release_identity(version: &Version) -> String {
    workflow_identity(RELEASE_WORKFLOW, &format!("refs/tags/v{version}"))
}

fn release_has_attestation(version: &Version) -> bool {
    // 1.0.3 was backported after attestation was introduced
    version >= &FIRST_ATTESTED_RELEASE || version == &Version::new(1, 0, 3)
}

fn nightly_identity() -> String {
    workflow_identity(NIGHTLY_WORKFLOW, NIGHTLY_GIT_REF)
}

fn workflow_identity(workflow: &str, git_ref: &str) -> String {
    format!("https://github.com/{GITHUB_REPOSITORY}/{workflow}@{git_ref}")
}

fn nightly_source_uri() -> String {
    format!("git+https://github.com/{GITHUB_REPOSITORY}@{NIGHTLY_GIT_REF}")
}

fn verify(
    path: &Path,
    expected_identity: &str,
    expected_nightly: Option<(&str, &str)>,
) -> Result<()> {
    let digest = sha256_file(path)?;
    let attestations = fetch_attestations(digest).with_context(|| {
        format!(
            "Fetching build provenance attestations for `{}`",
            path.display()
        )
    })?;
    verify_attestations(&attestations, digest, expected_identity, expected_nightly)
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
    expected_nightly: Option<(&str, &str)>,
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
            if let Some(expected) = expected_nightly {
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

fn require_nightly_provenance(
    statement: &Statement,
    digest: Sha256Hash,
    expected: (&str, &str),
) -> Result<()> {
    let (source_commit, expected_subject) = expected;
    let source_matches = statement
        .predicate
        .pointer("/buildDefinition/resolvedDependencies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|dependency| {
            dependency.get("uri").and_then(Value::as_str) == Some(nightly_source_uri().as_str())
                && dependency
                    .pointer("/digest/gitCommit")
                    .and_then(Value::as_str)
                    == Some(source_commit)
        });
    if !source_matches {
        bail!(
            "Build provenance source commit does not match `{}`",
            source_commit
        );
    }

    let digest = digest.to_hex();
    let subject_matches = statement.subject.iter().any(|subject| {
        subject.name == expected_subject
            && subject.digest.sha256.as_deref() == Some(digest.as_str())
    });
    if !subject_matches {
        bail!(
            "Build provenance subject does not match `{}` with digest `sha256:{digest}`",
            expected_subject
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
    use super::*;

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

        verify_attestations(&attestations, digest, &nightly_identity(), None).unwrap();

        let wrong_digest = Sha256Hash::from_bytes([0; 32]);
        assert!(
            verify_attestations(&attestations, wrong_digest, &nightly_identity(), None).is_err()
        );
        assert!(verify_attestations(
            &attestations,
            digest,
            &release_identity(&Version::new(1, 1, 2)),
            None,
        )
        .is_err());
    }

    #[test]
    fn nightly_provenance_must_match_source_commit_and_subject() {
        let digest = Sha256Hash::from_bytes([7; 32]);
        let digest_hex = digest.to_hex();
        let statement = serde_json::from_value::<Statement>(serde_json::json!({
            "_type": "https://in-toto.io/Statement/v1",
            "subject": [{
                "name": "anchor-nightly-20260803-abcdef0-aarch64-apple-darwin.tar.gz",
                "digest": { "sha256": digest_hex }
            }],
            "predicateType": SLSA_PROVENANCE_V1,
            "predicate": {
                "buildDefinition": {
                    "resolvedDependencies": [{
                        "uri": nightly_source_uri(),
                        "digest": { "gitCommit": "abcdef0123456789" }
                    }]
                }
            }
        }))
        .unwrap();
        let expected = (
            "abcdef0123456789",
            "anchor-nightly-20260803-abcdef0-aarch64-apple-darwin.tar.gz",
        );

        require_nightly_provenance(&statement, digest, expected).unwrap();

        assert!(
            require_nightly_provenance(&statement, digest, ("0000000000000000", expected.1))
                .is_err()
        );
        assert!(require_nightly_provenance(
            &statement,
            digest,
            (
                expected.0,
                "anchor-nightly-20260802-0000000-aarch64-apple-darwin.tar.gz"
            )
        )
        .is_err());

        assert!(
            require_nightly_provenance(&statement, Sha256Hash::from_bytes([8; 32]), expected)
                .is_err()
        );
    }
}
