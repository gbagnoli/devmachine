//! Bounded TCG EFI crypto-agile log replay for SHA256 PCR 7 only.
use super::{extend, invalid, Result};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;

pub(super) struct Replay {
    pub pcr7: [u8; 32],
    pub authority_seen: bool,
    pub secure_boot: bool,
}

struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let (value, rest) = self
            .0
            .split_at_checked(n)
            .ok_or_else(|| invalid("truncated firmware event log"))?;
        self.0 = rest;
        Ok(value)
    }
    fn u32(&mut self) -> Result<u32> {
        let bytes = self
            .take(4)?
            .try_into()
            .map_err(|_| invalid("invalid event length"))?;
        Ok(u32::from_le_bytes(bytes))
    }
    fn u16(&mut self) -> Result<u16> {
        let bytes = self
            .take(2)?
            .try_into()
            .map_err(|_| invalid("invalid event algorithm"))?;
        Ok(u16::from_le_bytes(bytes))
    }
    fn length(&mut self) -> Result<usize> {
        usize::try_from(self.u32()?).map_err(|_| invalid("event length overflow"))
    }
    fn data(&mut self) -> Result<&'a [u8]> {
        let size = self.length()?;
        self.take(size)
    }
}

pub(super) fn replay(log: &[u8], authority: &[u8]) -> Result<Replay> {
    if log.len() > 8 * 1024 * 1024 {
        return Err(invalid("firmware log exceeds the size limit"));
    }
    let mut cursor = Cursor(log);
    if cursor.u32()? != 0 || cursor.u32()? != 3 {
        return Err(invalid("missing TCG EFI Spec ID header"));
    }
    cursor.take(20)?;
    let mut spec = Cursor(cursor.data()?);
    if spec.take(16)? != b"Spec ID Event03\0" {
        return Err(invalid("unsupported firmware event log format"));
    }
    spec.take(8)?;
    let count = spec.length()?;
    if !(1..=16).contains(&count) {
        return Err(invalid("invalid firmware digest algorithm count"));
    }
    let mut sizes = BTreeMap::new();
    for _ in 0..count {
        let algorithm = spec.u16()?;
        let size = usize::from(spec.u16()?);
        if !(1..=64).contains(&size) || sizes.insert(algorithm, size).is_some() {
            return Err(invalid("invalid firmware digest algorithm table"));
        }
    }
    if sizes.get(&11) != Some(&32) {
        return Err(invalid("firmware log has no SHA256 bank"));
    }
    let mut result = Replay {
        pcr7: [0; 32],
        authority_seen: false,
        secure_boot: false,
    };
    while !cursor.0.is_empty() {
        replay_event(&mut cursor, &sizes, authority, &mut result)?;
    }
    Ok(result)
}

fn replay_event(
    cursor: &mut Cursor<'_>,
    sizes: &BTreeMap<u16, usize>,
    authority: &[u8],
    replay: &mut Replay,
) -> Result<()> {
    let pcr = cursor.u32()?;
    let kind = cursor.u32()?;
    let count = cursor.length()?;
    if count == 0 || count > sizes.len() {
        return Err(invalid("invalid firmware event digest count"));
    }
    let mut digests = BTreeMap::new();
    for _ in 0..count {
        let algorithm = cursor.u16()?;
        let length = *sizes
            .get(&algorithm)
            .ok_or_else(|| invalid("unknown digest algorithm"))?;
        if digests.insert(algorithm, cursor.take(length)?).is_some() {
            return Err(invalid("duplicate event digest algorithm"));
        }
    }
    let data = cursor.data()?;
    if pcr == 7 && kind != 3 {
        let digest: [u8; 32] = digests
            .get(&11)
            .copied()
            .ok_or_else(|| invalid("missing PCR 7 SHA256 digest"))?
            .try_into()
            .map_err(|_| invalid("invalid SHA256 digest"))?;
        if digest != <[u8; 32]>::from(Sha256::digest(data)) {
            return Err(invalid("PCR 7 event payload does not match its digest"));
        }
        replay.pcr7 = extend(replay.pcr7, digest);
        replay.authority_seen |= kind == 0x8000_00e0 && data == authority;
        if kind == 0x8000_0001 && secure_boot_event(data) {
            replay.secure_boot = true;
        }
    }
    Ok(())
}

fn secure_boot_event(data: &[u8]) -> bool {
    // EFI_GLOBAL_VARIABLE GUID + UnicodeNameLength + VariableDataLength.
    let guid = [
        0x61, 0xdf, 0xe4, 0x8b, 0xca, 0x93, 0xd2, 0x11, 0xaa, 0x0d, 0x00, 0xe0, 0x98, 0x03, 0x2b,
        0x8c,
    ];
    let mut expected = guid.to_vec();
    expected.extend(10_u64.to_le_bytes());
    expected.extend(1_u64.to_le_bytes());
    for c in "SecureBoot".encode_utf16() {
        expected.extend(c.to_le_bytes());
    }
    expected.push(1);
    data == expected
}
