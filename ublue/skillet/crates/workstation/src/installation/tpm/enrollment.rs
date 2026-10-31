//! Retryable enrollment policy, independent of filesystem/process adapters.
use super::{invalid, validate_jwe, Policy, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct State {
    pub version: u8,
    pub volume: String,
    pub policy: Policy,
    pub source_slot: u8,
    pub target_slot: u8,
    pub target_token: u8,
    pub source_jwe: String,
    pub target_jwe: String,
    pub verified_target_boot: bool,
}

pub(super) struct Observation {
    pub volume: String,
    pub policy: Policy,
    pub metadata: Value,
}

/// Effects are confined to the fresh-install enrollment owner.
pub(super) trait Resource {
    fn observe(&mut self) -> Result<Observation>;
    fn load(&mut self) -> Result<Option<State>>;
    fn save(&mut self, state: &State) -> Result<()>;
    fn seal(&mut self, source: &str, target: [u8; 32]) -> Result<String>;
    fn install(&mut self, state: &State) -> Result<()>;
    fn verify(&mut self, state: &State, target_boot: bool) -> Result<()>;
}

pub(super) fn enroll(resource: &mut impl Resource) -> Result<bool> {
    let observation = resource.observe()?;
    let current = observation.policy.initial;
    let mut state = if let Some(state) = resource.load()? {
        validate_state(&state, &observation)?;
        if state.verified_target_boot {
            // Installation is complete. Later operator key management must not
            // cause routine boots to recreate removed slots or rotate bindings.
            return Ok(true);
        }
        state
    } else {
        let (slot, source) = initial_binding(&observation.metadata, observation.policy.initial)?;
        let already_measured = observation.policy.initial == observation.policy.target;
        let target_slot = if already_measured {
            slot
        } else {
            free_id(&observation.metadata["keyslots"])?
        };
        let target_token = free_id(&observation.metadata["tokens"])?;
        let target_jwe = if already_measured {
            source.clone()
        } else {
            resource.seal(&source, observation.policy.target)?
        };
        validate_jwe(&target_jwe, observation.policy.target)?;
        let state = State {
            version: 1,
            volume: observation.volume,
            policy: observation.policy,
            source_slot: slot,
            target_slot,
            target_token,
            source_jwe: source,
            target_jwe,
            verified_target_boot: false,
        };
        // Both keys remain TPM-sealed; persist ciphertext before any LUKS mutation.
        resource.save(&state)?;
        state
    };
    let target_boot = current == state.policy.target;
    if state.source_slot != state.target_slot {
        resource.install(&state)?;
    }
    resource.verify(&state, target_boot)?;
    if target_boot && !state.verified_target_boot {
        state.verified_target_boot = true;
        resource.save(&state)?;
    }
    Ok(state.verified_target_boot)
}

fn validate_state(state: &State, observation: &Observation) -> Result<()> {
    if state.version != 1
        || state.volume != observation.volume
        || state.policy.certificate_sha256 != observation.policy.certificate_sha256
        || (state.source_slot == state.target_slot
            && (state.policy.initial != state.policy.target
                || state.source_jwe != state.target_jwe))
        || state.source_slot >= 32
        || state.target_slot >= 32
        || state.target_token >= 32
        || ![state.policy.initial, state.policy.target].contains(&observation.policy.initial)
    {
        return Err(invalid(
            "retained enrollment does not match this volume, certificate or boot state",
        ));
    }
    validate_jwe(&state.source_jwe, state.policy.initial)?;
    validate_jwe(&state.target_jwe, state.policy.target)
}

fn initial_binding(metadata: &Value, current: [u8; 32]) -> Result<(u8, String)> {
    let slots = metadata["keyslots"]
        .as_object()
        .ok_or_else(|| invalid("missing LUKS2 slots"))?;
    let tokens = metadata["tokens"]
        .as_object()
        .ok_or_else(|| invalid("missing LUKS2 tokens"))?;
    if slots.len() != 1 || tokens.len() != 1 {
        return Err(invalid(
            "initial transition enrollment requires the fresh single-binding root volume",
        ));
    }
    let slot = slots
        .keys()
        .next()
        .ok_or_else(|| invalid("missing root slot"))?
        .parse::<u8>()
        .map_err(|_| invalid("invalid root slot"))?;
    if slot >= 32 {
        return Err(invalid("invalid root slot"));
    }
    let token = tokens
        .values()
        .next()
        .ok_or_else(|| invalid("missing initial root token"))?;
    if token["type"] != "clevis" || token["keyslots"] != serde_json::json!([slot.to_string()]) {
        return Err(invalid(
            "initial root token is not the expected Clevis binding",
        ));
    }
    let jwe = compact_jwe(&token["jwe"])?;
    validate_jwe(&jwe, current)?;
    Ok((slot, jwe))
}

fn free_id(value: &Value) -> Result<u8> {
    let entries = value
        .as_object()
        .ok_or_else(|| invalid("invalid LUKS2 metadata"))?;
    (0_u8..32)
        .find(|id| !entries.contains_key(&id.to_string()))
        .ok_or_else(|| invalid("no free LUKS2 slot/token"))
}

pub(super) fn compact_jwe(value: &Value) -> Result<String> {
    if let Some(compact) = value.as_str() {
        return Ok(compact.to_owned());
    }
    let fields = ["protected", "encrypted_key", "iv", "ciphertext", "tag"];
    let mut values = Vec::new();
    for field in fields {
        if field == "encrypted_key" && value.get(field).is_none() {
            values.push("");
        } else {
            values.push(
                value[field]
                    .as_str()
                    .ok_or_else(|| invalid("invalid Clevis JWE serialization"))?,
            );
        }
    }
    Ok(values.join("."))
}

pub(super) fn token(state: &State) -> Result<Value> {
    let fields: Vec<_> = state.target_jwe.split('.').collect();
    let [protected, encrypted_key, iv, ciphertext, tag] = fields.as_slice() else {
        return Err(invalid("invalid compact Clevis JWE"));
    };
    Ok(serde_json::json!({
        "type": "clevis", "keyslots": [state.target_slot.to_string()],
        "jwe": { "protected": protected, "encrypted_key": encrypted_key,
            "iv": iv, "ciphertext": ciphertext, "tag": tag }
    }))
}
