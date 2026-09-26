# Agent assignment 2: reproducible clamps VM boot

Draft assignment; writing this file does not launch an agent. Suggested model: `gpt-6-sol`, medium reasoning, fresh context with this assignment and the migration draft. Start implementation after agent 1 provides the base-phase interface and artifact. The parent reviews changes and acceptance evidence.

## Objective

From `ublue/butane/`, one documented invocation of `./bin/coreos-install clamps.bu` provisions a disposable VM which reaches the signed uCore clamps deployment and automatically completes the Skillet base phase. The agent may use virsh, SSH, serial logs, guest commands, reboot, and repeated Skillet applies to diagnose and validate it.

Final acceptance must come from a fresh install with fixes encoded in the source. Manual guest repairs alone do not count. This assignment proves VM boot, not physical hardware/Secure Boot, TPM credentials, Pi-hole parity, production storage or cutover.

## Ownership and preflight

- Own `ublue/butane/` and its documentation/tests. Shared `ublue/lib/` changes require parent coordination. Agent 1 owns `ublue/skillet/`; request Rust fixes through that agent/parent.
- Read the migration draft, launcher, Butane wrapper, all clamps includes, host-execution helpers, and agent 1's handoff.
- Existing scripts use a Flatpak virt-install wrapper, user networking without an explicit inbound SSH path, a domain named `clamps`, and unconditional deletion of `images/clamps.img`. Fix collision handling before executing them against existing artifacts.
- Identify the actual libvirt URI and execution environment. Use the same URI for creation, inspection and cleanup; account for host commands when inside distrobox. Check permissions, KVM, disk space, RAM, Podman, Butane and SSH access. Host tools existing on PATH is not proof of working virtualization.
- Use only newly created disposable VM disks. Refuse to overwrite an existing domain or disk unless it is identified as belonging to this test and replacement was explicitly requested. Preserve any pre-existing `clamps` VM/image. Support a unique VM-name/output option while retaining the simple invocation on a clean environment.
- Report permission/environment blocks with the failing command. Use normal escalation when required; do not silently change host networking, security settings, or unrelated VMs.

## Agreed handoff from agent 1

- A static host binary `skillet-clamps` supporting `apply --phase base`, with a build command and SHA256.
- The default full apply is unchanged and requires application credentials where configured.
- A smoke check accepting an explicit disposable SSH target, using real systemd/Podman and dummy fixtures.

## Implementation plan

1. Make the launcher safe to repeat. Parse inputs reliably, isolate generated artifacts, name the VM explicitly, and reject collisions before replacing anything. Provide intentional cleanup/recreate for test-owned resources. Show the exact VM identity and output paths.
2. Establish unattended SSH access. Prefer loopback-bound forwarding on the existing user-network setup if supported; otherwise use a documented isolated libvirt network. Use test-specific SSH configuration and known_hosts. Permit existing authorized credentials or generate an ignored ephemeral test key; embed only its public key. Avoid interactive password-change prompts in test access.
3. Keep `/dev/vda` specific to the VM layout. Separate or clearly guard physical-install disk selection. Do not choose the final `/srv` design here.
4. Build/stage the supplied Skillet artifact automatically as part of provisioning, or accept a documented artifact override. For this milestone, prefer an Ignition-delivered artifact under `/var/lib/skillet/` with executable permissions and a persistent unit invoking it. Verify execution with SELinux enforcing. Do not rely on modifying immutable `/usr`, publishing a new image, or manually copying the binary after first boot.
5. Provision bootstrap DNS before the first rebase into the resolved-masked image. Use a single owner for resolver state, obtain suitable upstreams from the test network, and avoid dependence on the future Pi-hole service or a nonexistent resolved runtime file. Confirm NetworkManager behavior on the actual guest. Do not hardcode production LAN values as a convenience.
6. Make rebase stages recoverable and sequenced. Distinguish requested/pending deployments from the actually booted deployment using rpm-ostree state. Verify the expected signed origin and signature policy, not just marker files or an image name. Prevent Skillet starting before the intended booted deployment. Bound failure/retry behavior so a failed rebase cannot create a reboot loop.
7. Configure `skillet-apply.service` to invoke the explicit base phase without application credentials. Arrange reruns/recovery after failure. Keep all required baseline files, access and binary delivery valid across both rebase boots.
8. Add an automated readiness/verification command using virsh plus SSH/serial diagnostics. Poll with deadlines; report the failing stage and journals. A successful virt-install exit or SSH response alone is not readiness.

The local image checkout is at `759f195` as of drafting. Preserve its common-before-host build dependency. If the published image cannot support the task, report the exact mismatch and proposed image change to the parent; do not silently publish or weaken verification.

## Exit criteria

| Case | Evidence required |
| --- | --- |
| Clean install | The documented launcher invocation completes provisioning from a fresh VM disk with no manual guest repair; exact input revisions and artifact SHA recorded |
| Access | virsh identifies the expected domain and disk; noninteractive SSH and needed sudo work using the documented connection method |
| Rebase sequence | Boot IDs and deployment/journal evidence show the intended transitions and the final booted signed clamps deployment; no pending bootstrap transition or repeating rebase |
| Resolver continuity | External resolution and registry access work at the stages that need them and after final reboot; resolved remains masked on final uCore |
| Artifact delivery | Guest binary SHA matches the staged artifact and it executes under enforcing SELinux after rebases |
| Automatic apply | Skillet unit finishes successfully on the intended deployment without application credentials or an SSH-triggered first apply |
| Repeat apply | Two further base applies succeed without unnecessary file changes or service restarts; use actual state evidence |
| Reboot | Another ordinary reboot retains SSH, DNS, binary and successful baseline state; bootstrap does not restart the rebase cycle |
| Interruption | Interrupt a disposable bootstrap at a recorded stage; the subsequent boot recovers to the expected final state without hand-editing markers |
| Repeatability | A second fresh VM creation from the final source passes the same checks; existing unrelated VM/disk collision is refused safely |
| Real Skillet scenario | Run agent 1's fixture checks on final uCore, including repeat apply, failure recovery, secret rotation and reboot |

Run strict Butane validation, shell syntax checks and ShellCheck for changed scripts when available. Rerun the end-to-end installation after the last bootstrap-affecting edit. Capture unexpected failed units and resolve those caused by the change; disclose unrelated failures.

## Deliverables

- Source changes and concise instructions: clean provision, connection details, readiness check, base apply, log collection and explicit test cleanup.
- A result for every exit criterion: pass, fail, or not run, with reproducible commands and local evidence paths. Include actual image digests, tool versions, guest boot IDs and final deployment status.
- Diagnostics for failed runs without credential material. Track generated keys, Ignition, VM disks and logs outside committed source.
- Leave the successful VM inspectable and report its domain/URI/connection command. Clean up only test-owned resources when explicitly requested or according to documented test lifecycle.

Return to the parent for review when criteria pass. Report environment or unavailable-image blockers accurately; never label an unexecuted acceptance check as passed. No nested delegation, production changes, physical disk operations or image publishing.
