# Skillet releases

Run commands from `ublue/skillet`. Python 3.11+, Git, Cargo and the exact
`cargo-release` version in `tools/cargo-release-version` must be available on
`PATH`. The launcher checks that version before making changes; it installs nothing.
Use a cargo-release executable compatible with your host's glibc, or run the
tools together in a compatible container/toolbox.
Upstream performs read-only crates.io name lookups even with publishing disabled.

1. Commit the intended source changes, run the normal local checks and start
   from a clean `master` checkout. Preview the release:

   ```sh
   python3 tools/cut_release.py patch
   ```

   Use `minor`, `major`, `rc`, or an explicit version instead of `patch` when
   appropriate. The default is a dry run. All workspace crates inherit one version.
2. Review the preview, then execute locally. Executed releases first run every
   Skillet CI check, including Podman acceptance and Butane/bootstrap checks.
   `tools/ci-checks.json` is shared with CI; failures prevent release mutation.
   All existing CI runtime prerequisites must be available; no tools are installed.
   Dry-run previews skip these checks. To run the suite separately, use
   `python3 tools/ci_checks.py`. Other repository lint and release-publication jobs
   still run in GitHub Actions.

   Execute locally:

   ```sh
   python3 tools/cut_release.py patch --execute
   python3 tools/release.py --tag skillet-v0.1.1
   ```

   Adjust the example tag to the resulting version. cargo-release updates the
   workspace version and lockfile, creates one release commit and one annotated
   `skillet-vVERSION` tag. It neither publishes to crates.io nor pushes. The
   launcher ignores implicit cargo-release configuration and uses `release.toml`.
   Never move/reuse a release tag. A partial failure may leave edits: inspect
   Git status and tags before retrying rather than automatically deleting them.
3. Before publishing the first release, enable **Settings → General → Releases →
   Enable release immutability** in devmachine. This is a GitHub repository setting;
   the workflow does not configure it. See [GitHub's instructions](https://docs.github.com/en/code-security/how-tos/secure-your-supply-chain/establish-provenance-and-integrity/prevent-release-changes).
4. Push the commit and then its tag. A tag push starts `Skillet release`: it checks
   the tag/version, reuses full CI without path-based skips, builds static binaries,
   verifies every host bundle, then publishes the source release. Only publication
   receives repository write permission. Existing releases are never overwritten.
   Stable releases then open a dependency PR in ucore-images automatically;
   complete the one-time [CI credential setup](#automatic-image-dependency-pr)
   before pushing the tag.
5. The release workflow waits for the generated ucore-images PR's required
   checks, then merges it with the cross-repository token. Image CI builds the
   host images from the exact published release.

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

For a prerelease, use `python3 tools/cut_release.py 0.2.0-rc.1 --execute`;
the workflow marks its GitHub release as a prerelease.
Image consumers must explicitly select it in their dependency lock.

CI tests the pinned tool against temporary Git repositories and a local bare
remote: dry-run preservation, patch/prerelease tags, lockfile consistency,
branch refusal and unchanged remote refs. Run that check explicitly with
`python3 tools/tests/cargo_release_acceptance.py` in a compatible tool environment.

To check locally built release binaries too, add
`--binary-dir target/x86_64-unknown-linux-musl/release` to the validator command.
Artifact paths follow the explicit Cargo target directory used for that build.

Public bundle commands: [image bundles](image-bundles.md).
Compatibility policy: [design](../../../../design/skillet-releases.md).
