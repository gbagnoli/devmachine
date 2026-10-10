# 6. Explicit convergence outcomes and failure-safe recording

Status: implementation complete; fixture failure-recovery/reboot, host
credential rotation, and unchanged repeat-apply acceptance passed. See the
[prerequisite roadmap](SKILLET-REFACTOR.md).

## Read and locate

- `../skillet/crates/core/src/{recorder,resource_op,test_utils}.rs` and the
  resource traits revised in workstream 3.
- `../skillet/crates/hardening/src/lib.rs`, Podman revision/activation logic,
  btrbk unit convergence, and apply/record handling in the CLI libraries.
- `../design/{skillet-architecture,smoke-vms}.md` and existing failure-recovery
  unit/VM fixture tests.

Original defects: activation markers were implemented separately in hardening
and Podman; returned booleans described file changes despite restarts.
Recordings were saved only after successful apply and exposed payload hashes.

## Progress, 2026-10-04

- Implemented one shared activation helper for hardening, Podman containers,
  and the btrbk timer. It reports definition changes, starts/restarts, and
  recovery of pending activation; marker state is committed only after a
  successful service action. Persistent services are restarted after desired
  changes, stopped unchanged services are started, and completed oneshots are
  not rerun just because they are inactive.
- Implemented version 1 diagnostic recordings with overall outcome and
  per-operation success/change/failure. Recording omits file contents, content
  hashes, secret payload fingerprints, and raw operation errors. Writes use a
  synced temporary file and atomic replacement. Failed applies are recorded;
  if recording also fails, the returned error retains both causes.
- Focused tests cover changed/no-op/stopped states, reload/start/restart
  interruption and retry, oneshots, secret redaction, failed apply recording,
  and simultaneous apply/record failure. The fresh VM fixture passed its
  failure-recovery and reboot cases. A further full host apply preserved the
  service containers' start timestamps, confirming unchanged repeat apply.

## Implementation sequence

1. Specify result contracts before editing callers. Low-level file/secret
   results describe their own convergence; aggregate apply outcomes distinguish
   definition change, start/restart/recovery and no-op. Use a small documented
   result type where a boolean cannot express the operation. Audit every caller
   and test that currently interprets the boolean.
2. Extract a focused activation helper for desired revision, pending applied
   state, required daemon reload, consumer action, and successful marker commit.
   Keep resource-specific revision inputs and desired active-state policy in
   the caller. Distinguish oneshot actions from persistent services; do not
   start completed oneshots repeatedly merely because they are inactive.
3. Preserve interruption behavior: files may already match after a failed reload
   or restart, but pending activation must retry. Write applied state only after
   success. Account for network-global policy and application input/secret
   revisions. Import existing markers or document a bounded migration; never
   erase state merely to make a second apply appear clean.
4. Update recording to capture attempted operation plus sanitized result,
   relevant non-secret parameters and overall apply outcome. Include account IDs
   and consistent ownership types. Record failure without raw payloads, credential
   bytes or unsafe subprocess output; fingerprints are not plaintext substitutes
   to log indiscriminately. Do not record private configuration contents.
5. Persist the recording even when apply fails. Preserve the original apply
   error if recording persistence also fails, with a separate useful diagnostic.
   Use atomic writes or another explicitly recoverable format. Version format
   changes and document compatibility for existing diagnostic consumers.
6. Update consumers, fakes, tests and README descriptions together. Recording
   remains observational: it executes the real resources and is neither a dry
   run nor acceptance proof. Update AGENTS result/recording prevention rules.

## Validation and exit criteria

- Tests cover changed definition, consumed-input and secret-only changes,
  stopped service, genuine no-op, failed reload/start/restart and successful
  retry. Assert markers remain pending on failure and commit after success.
- Oneshot and persistent consumers obey different activity policies. Repeat
  apply leaves an unchanged running container's identity/start time intact.
- A failed apply produces a readable diagnostic recording containing the failed
  operation and prior results. No-op and successful changes are distinguishable;
  combined apply/record-write failure retains the original cause.
- Tests using distinctive dummy payloads verify secrets and private configuration
  bytes never appear in recordings or returned diagnostic messages.
- Run the real VM fixture's configuration/secret/failure/reboot cases and an
  affected host repeat apply. Pass roadmap checks, retain evidence, and document
  implemented marker/record format behavior before completion.
