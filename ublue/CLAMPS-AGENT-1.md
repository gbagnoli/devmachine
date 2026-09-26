# Agent assignment 1: reliable Skillet convergence

Draft assignment; writing this file does not launch an agent. Suggested model: `gpt-6-sol`, medium reasoning, fresh context with this assignment and the migration draft. The parent reviews the implementation and evidence.

## Objective

Make Skillet's existing resources reliably converge under real systemd and Podman. Deliver a repeatable smoke check for agent 2's disposable uCore VM. Do not add production services or implement the later Pi-hole/storage/TPM migration.

Read `ublue/CLAMPS-MIGRATION.md`, `ublue/skillet/AGENTS.md`, and the relevant Rust implementations before editing. Resolve applicable instructions from the actual checkout. Preserve unrelated changes.

## Ownership and interface

- Own `ublue/skillet/`, including tests, documentation, and its integration-test instructions. Coordinate any edit outside that directory with the parent.
- Agent 2 owns `ublue/butane/`. Do not change its launcher or bootstrap units.
- Preserve normal full-apply behavior. Add an explicit `apply --phase base` option to both the generic and clamps binaries. The base phase applies only the host baseline and must not read application credentials or start Pi-hole. Default `apply` continues to mean the full configured host; missing required credentials must remain an error there.
- Both entry points must call the same baseline implementation. Keep this a small explicit phase selection; no general dependency scheduler or plugin framework.
- Produce the static `skillet-clamps` artifact and document its exact build command, path and SHA256. Agent 2 supplies it to the guest and invokes `skillet-clamps apply --phase base`.
- Provide a documented smoke command against an explicit SSH target. It must fail closed without a disposable-target opt-in and never default to the workstation or production clamps. Agent 2 owns VM creation; this check owns guest assertions.

## Implementation plan

1. Fix service start/restart calls to wait for startup completion and report failures. Remove the blanket `--wait` use, which waits for service termination. Bound smoke-test waits and retain failure output.
2. Correct generated Quadlet lifecycle: use `[Install]` plus generator reload for persistence, not `systemctl enable` on generated units. Ensure inactive desired services start even when files are unchanged. Preserve ordinary unit enablement where appropriate.
3. Track and recover unfinished convergence. A configuration write followed by failed reload/restart must be retried on the next apply. Avoid a change flag that disappears across runs before the service consumes the change.
4. Propagate changes to consumed secrets into container recreation. Recover from interrupted secret updates; a failed inspect must not automatically be interpreted as 'missing'. Do not expose secret values in logs, arguments, recordings or errors.
5. Fix ownership/mode churn on paths managed by both application and container resources. Add only the primitives needed for these cases.
6. Implement the base-phase interface above. Cover both CLI entry points and retain normal full-apply errors.
7. Replace recording equality as the acceptance oracle. Keep recordings only if diagnostically useful; remove obsolete test commands/docs if scrapping them. Update `AGENTS.md` explicitly to replace the old container-recording requirement with unit checks plus the real-runtime scenario.

## Smoke scenario and exit criteria

Use a harmless fixture container configured through the same Rust resources used by hosts. Do not start the partial production Pi-hole implementation for this milestone. Use dummy secrets and a persistent test volume. Mock systemctl/Podman is forbidden for runtime acceptance; unit tests may use mocks.

| Case | Required observation |
| --- | --- |
| First apply | Returns within a documented deadline; service is running; fixture reports expected configuration; volume is usable |
| Repeat apply | Managed bytes, permissions and ownership are unchanged; container identity/start timestamp is unchanged |
| Stopped service | Apply starts it despite unchanged configuration |
| Configuration change | Running fixture consumes the new configuration; subsequent apply causes no further recreation |
| Secret rotation | Running fixture consumes the replacement dummy secret; verification reports pass/fail without printing it |
| Startup failure | Apply returns nonzero and useful diagnostics; fixing the cause and reapplying identical desired configuration succeeds |
| Interrupted convergence | Inject failure after writing desired state but before successful activation; a new apply completes pending work |
| Reboot | Fixture starts automatically; persistent test data remains; another apply succeeds without unnecessary restart |
| Base/full separation | Base succeeds without credentials; full clamps apply still rejects missing required credentials |

Capture hashes, ownership/modes, systemd result and container identity before/after. An unchanged operation count is not enough. Check content and runtime state. Fixture namespaces, files and services must be distinctive and removable without touching other guest workloads.

Run formatting, workspace tests, and pedantic Clippy with warnings denied, using the workspace's musl configuration. Tests belong in separate modules. Preserve `thiserror` in libraries, no `unsafe`, no library `unwrap`/`expect`, and crate-first system interactions.

## Handoff and completion

Two statuses are deliberately distinct:

- **Ready for VM integration:** implementation, focused unit tests, lint/build checks, artifact and smoke command are ready. Include any runtime checks already performed. Agent 2 may start from this handoff.
- **Accepted:** all smoke cases have passed with real systemd/Podman, including on agent 2's final uCore VM, and the parent has reviewed the changes. If a guest is unavailable, report pending runtime validation; do not claim acceptance.

Return changed files, interface/build commands, artifact hash, test results, evidence paths, and unresolved issues. Agent 2 reports Skillet defects to this agent/parent instead of editing owned files concurrently. No nested delegation, production changes, publishing or physical disk operations.
