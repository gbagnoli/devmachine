# Fleet host declarations

Rupik is replaced by clamps, boxy by joey and calculon by donbot. Canonical callers
own service/UI/credential/boot/acceptance selection; shared recipes own behavior.
Baseline services are common policy, optional workloads require explicit selection.

Names agree across profile, binary/package, Butane and image. The identity rename
provides no aliases and does not silently rewrite retained manifests, provider
journals or vault entries. Historical evidence retains tested identities; existing
resources require explicit owned cleanup/migration before new identities are used.

Generic units read /etc/skillet/host independently of runtime VM hostname. Activation
units must load the complete credential set for their caller's capabilities.
Shared storage/layout does not establish live source parity or private route policy.

The selected resolver topology has clamps and joey colocated, plus donbot at
another site. The local pair covers individual-host failure but shares power
and internet failure; a remote resolver adds a distinct site failure domain.
This is independent of Cloudflare publication of private UI names.

[Current profiles and module guides](../skillet/crates/host-profiles/README.md).
Pending host work: [clamps](../plan/CLAMPS-MIGRATION.md),
[joey](../plan/JOEY-MIGRATION.md), [donbot](../plan/DONBOT-MIGRATION.md).
