# 7. Test layers, fixtures, build artifacts and CI

Status: implementation and required CI/fresh/retained VM acceptance complete.
Live legacy import and controlled interrupted-recovery checks passed; see the
[prerequisite roadmap](SKILLET-REFACTOR.md).

## Read and locate

- `../skillet/integration_tests/{smoke-ssh,smoke-guest}.sh`, the synthetic host
  fixture, CLI container runner and `src/test_entrypoint.sh`.
- Revised resource fakes, VM manifest/transport, profiles, provisioning and
  recording interfaces from workstreams 1–6.
- `../../.github/workflows/ci.yml`, `../butane/tests/bootstrap.sh`, both READMEs,
  `../design/smoke-vms.md` and `../butane/ACCEPTANCE.md`.

Original defects: fixture smoke, host acceptance and mocked container checks
were conflated; target/artifact selection assumed default paths/instances; fakes
disagreed with adapter behavior; extensionless helpers and Butane-only changes
missed relevant CI. Shell code was embedded in a production Rust fixture.

## Progress, 2026-10-04

- Moved the synthetic application out of the production CLI module into the
  dedicated `skillet-smoke-fixture` package and binary. Its guest entrypoint is
  a standalone `integration_tests/fixture-entrypoint.sh` file. The hidden
  production CLI fixture command was removed; the guest SSH smoke installs the
  fixture binary explicitly.
- Smoke derives Cargo target/profile from the invoked executable, builds the
  fixture through Cargo JSON artifact reporting, and refuses an unreported or
  missing executable. It verifies the fixture transfer and both installed
  guest binaries against SHA-256 values recorded in the VM manifest. A unit
  test covers custom target and release/debug selection. Existing VM identity,
  instance, manifest SSH target, and port remain separately validated.
- CI now selects Rust validation when Skillet, Butane, its helpers, or the
  workflow changes; it runs workspace formatting, all-target strict Clippy,
  all-target tests, and the named Fedora container integration command. Shell
  lint explicitly includes current extensionless scripts. Fresh/retained VM
  fixture and reboot passed. The live legacy-manifest import and controlled
  interrupted-provisioning recovery also passed; evidence is in the
  2026-10-05 acceptance entry.

## Implementation sequence

1. Document four acceptance scopes: pure/adapter-contract tests, binary apply
   checks in a container with fake runtime commands, real VM convergence fixture,
   and actual host application/live-provider acceptance. Recording is a diagnostic
   aid, not a fifth acceptance layer. Preserve useful distinct layers; remove a
   redundant check only after equivalent meaningful coverage is demonstrated.
2. Move the synthetic application into a separate test fixture binary/package
   outside production host dispatch. Keep its guest program in a standalone
   script file; no Rust shell string constant. Use the production resource engine
   so configuration, secret, failure-retry and reboot tests remain representative.
   Guest assertions may stay standalone Bash and get explicit validated inputs.
3. Select VM smoke targets using host plus instance and the validated manifest
   from workstream 1. Resolve key, host-key file, port and installed artifacts
   together. Preserve existing default command behavior through delegation; keep
   any explicit remote-target mode clearly opted into a disposable target.
   Fixed production-name exclusions are not sufficient ownership validation.
4. Consolidate artifact building/discovery for create, update, smoke and container
   tests. Parse Cargo JSON artifact output and honor target/profile/target-directory
   configuration. Do not select a stale release/debug artifact by directory scan.
   Record hashes; make captured readiness artifacts versus current-code update
   explicit and verify generic/host/fixture versions used by a scenario.
5. Add focused fake/adapter contract tests. Account IDs and directory metadata
   must be observable; directory/subvolume checks and service/secret failures
   cannot silently pass. Shell stubs must match production exit/error semantics.
   Full host composition with injected inputs belongs in fast CI even though
   a Fedora sandbox cannot provide the real service data mount or systemd.
6. Add deterministic host acceptance scenarios selected by the profile's actual
   capabilities. Check running services, mounts/ownership, network mode and
   declared listeners, unchanged repeat apply, and persistence after reboot.
   Keep external ACME/enrollment scenarios opt-in and state what loopback probes
   establish. Do not silently label fixture success as actual application success.
7. Update CI to cover shared Rust, Butane and helper inputs; explicitly lint
   extensionless shell helpers and extracted fixtures. Add workspace formatting,
   pedantic Clippy including test targets, unit/contract checks, bootstrap state
   tests, and a clearly named container binary integration check. Routine CI
   needs no vault, live providers, hypervisor, or production network.
8. Update command docs and acceptance evidence. Correct stale status text and
   preserve deferred Pi-hole LAN, external-network UI, production ACME, backup
   restore and hardware checks in their existing plans. Test cases unavailable
   on a backend are unverified, not equivalent to a passing mock.

## Validation and exit criteria

- With a non-default target directory/profile and a named non-default instance,
  commands select the exact newly built artifacts and recorded SSH target.
  Missing/unsupported artifacts or ownership fail before guest mutation.
- Every important fake/adapter contract added in 3–6 has meaningful behavioral
  coverage, including changed observations and failed effects. Private state is
  unnecessary for routine CI; synthetic profiles are absent from production
  dispatch and no shell programs are embedded in Rust.
- CI selection includes relevant Butane/helper changes, all executable shell
  helpers are linted, and format/Clippy/unit/bootstrap/container commands pass
  locally. Update AGENTS with exact final commands and general coverage rules.
- A named fresh VM passed create/ready/provision, real convergence fixture,
  reboot, and external cleanup. A retained VM passed readiness, full
  provisioning, repeat apply, fixture failure recovery, and reboot. Live
  legacy-manifest import and controlled interrupted provisioning also passed.
- Finish only after earlier workstreams' exit criteria also pass. Update the
  roadmap gate and migration entry point so the next agent can resume feature
  work without losing deferred acceptance requirements.
