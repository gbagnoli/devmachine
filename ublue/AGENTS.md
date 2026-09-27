# uBlue agent instructions

## Design documentation

- Read the relevant documents in [design/](design/) before changing architecture
  or behavior they describe, including [secrets](design/secrets.md) and
  [smoke VM lifecycle](design/smoke-vms.md).
- Keep those documents updated in the same change as the implementation or
  decision they describe. Clearly distinguish planned from implemented behavior.
- Record new architectural decisions in `ublue/design/`, updating an existing
  document when appropriate or adding a short document for a new topic.
- Keep design documents brief: describe the decision, why it was chosen, and
  material consequences. Keep setup and usage instructions in READMEs, with
  links to the relevant designs instead of duplicated rationale.

## Implementation plans

- Keep implementation plans in [plan/](plan/), not in the `ublue/` root.
- Read relevant plans before starting work and update their remaining steps,
  status, and links as work progresses. Plans describe tasks and acceptance
  criteria; architectural decisions and rationale belong in `design/`.
- Remove completed or superseded plans after preserving any unfinished work
  in an active plan. Retain implementation evidence in the relevant acceptance
  record; do not label unverified checks as passed.
