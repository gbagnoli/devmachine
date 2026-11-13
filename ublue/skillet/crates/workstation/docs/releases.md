# Skillet releases

Run commands from `ublue/skillet` on a clean `master` checkout with Cargo,
Git and cargo-release available on PATH. No exact cargo-release version is required.

```sh
./release
```

The script runs workspace Clippy with warnings denied, then all workspace unit
tests. A failure stops it before `git push` or `cargo release`. Cargo-release
performs the patch bump, release commit, annotated tag and push; it does not
publish crates. The script installs nothing. CI runs the full validation suite
and publishes the source release, then proposes and merges the ucore-images update.

For a preview or a different version, invoke cargo-release directly:

```sh
cargo release minor --workspace --isolated --config release.toml --no-publish --no-push
```

Before publishing the first release, enable release immutability in GitHub.
Never move or reuse a release tag. Inspect partial failures before retrying.

## Automatic image dependency PR

In GitHub, create a **fine-grained personal access token** owned by an account
with write access to `ucore-images`. Restrict repository access to **ucore-images
only**, granting **Contents: Read and write** and **Pull requests: Read and write**.
The workflow uses this token to create the PR, wait for required checks and merge
the passing PR.
Metadata read access is implicit; workflow editing permission is unnecessary.

In **devmachine → Settings → Secrets and variables → Actions → New repository
secret**, save it as **`UCORE_IMAGES_PR_TOKEN`**. This is a CI secret, not a KeePass
entry. The default workflow token cannot write to the second repository. A PAT
also allows the resulting target-repository PR to run its normal CI checks.

After stable publication, CI verifies the tested lock and published release again,
then opens or updates `automation/skillet-vVERSION` against ucore-images `main`,
changing only `skillet-release.json`. The job summary links to the PR. A missing
or expired token fails the proposal job with publication already complete; fix
the secret and rerun **failed jobs**. Prereleases skip the proposal job and remain
an explicit image-consumer choice.

## Manual dependency update

For a prerelease or a manual fallback, from a clean checkout of the release:

```sh
python3 tools/release.py --tag skillet-v0.1.1 \
  --write-lock "$HOME/workspace/ucore-images/skillet-release.json"
```

Review and commit the version/commit change there, then push ucore-images.
Image CI checks the published release, annotated tag and Cargo version before
building from the locked commit. It verifies matching binary provenance and
transported bundles before the host image builds/signing.

For a prerelease, use `cargo release 0.2.0-rc.1 --workspace --isolated --config release.toml --no-publish --no-push --execute`;
the workflow marks its GitHub release as a prerelease.
Image consumers must explicitly select it in their dependency lock.

CI tests the currently installed tool against temporary Git repositories and a local bare
remote: dry-run preservation, patch/prerelease tags, lockfile consistency,
branch refusal and unchanged remote refs. Run that check explicitly with
`python3 tools/tests/cargo_release_acceptance.py` in a compatible tool environment.

To check locally built release binaries too, add
`--binary-dir target/x86_64-unknown-linux-musl/release` to the validator command.
Artifact paths follow the explicit Cargo target directory used for that build.

Public bundle commands: [image bundles](image-bundles.md).
Compatibility policy: [design](../../../../design/skillet-releases.md).
