//! Read-only, bounded NUT polling and `DogStatsD` datagram submission.
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    os::unix::net::UnixDatagram,
    path::Path,
    time::{Duration, Instant},
};
use thiserror::Error;

const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_LINE: usize = 4096;
const MAX_RESPONSE: usize = 65_536;
const VARIABLES: &[&str] = &[
    "battery.charge",
    "battery.runtime",
    "battery.voltage",
    "input.voltage",
    "input.frequency",
    "output.voltage",
    "output.frequency",
    "ups.load",
    "ups.realpower",
    "ups.temperature",
];
const FLAGS: &[(&str, &str)] = &[
    ("OL", "online"),
    ("OB", "on_battery"),
    ("LB", "low_battery"),
    ("FSD", "forced_shutdown"),
    ("RB", "replace_battery"),
    ("OVER", "overload"),
    ("OFF", "off"),
    ("BYPASS", "bypass"),
];

#[derive(Debug, Error)]
pub enum MetricsError {
    #[error("invalid UPS name")]
    InvalidName,
    #[error("invalid or incomplete NUT response")]
    Protocol,
    #[error("NUT polling deadline exceeded")]
    Deadline,
    #[error("NUT query failed: {0}")]
    Query(#[source] std::io::Error),
    #[error("DogStatsD submission failed: {0}")]
    Submit(#[source] std::io::Error),
}

#[derive(Debug, PartialEq)]
pub struct Snapshot {
    values: BTreeMap<String, f64>,
    status: Vec<String>,
}

pub fn validate_name(name: &str) -> Result<(), MetricsError> {
    if name.is_empty()
        || name.len() > 40
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        || !name.as_bytes()[0].is_ascii_alphanumeric()
    {
        return Err(MetricsError::InvalidName);
    }
    Ok(())
}

/// The transport owns live network observations. Parsing and rendering are pure.
pub fn collect(server: SocketAddr, ups: &str) -> Result<Snapshot, MetricsError> {
    validate_name(ups)?;
    let deadline = Instant::now() + TIMEOUT;
    let mut stream =
        TcpStream::connect_timeout(&server, Duration::from_secs(2)).map_err(MetricsError::Query)?;
    stream
        .set_write_timeout(Some(TIMEOUT))
        .map_err(MetricsError::Query)?;
    stream
        .write_all(format!("LIST VAR {ups}\n").as_bytes())
        .map_err(MetricsError::Query)?;
    let mut reader = BufReader::new(stream);
    let mut response = Vec::new();
    for _ in 0..256 {
        let line = read_line_until(&mut reader, deadline)?;
        if response.len() + line.len() > MAX_RESPONSE {
            return Err(MetricsError::Protocol);
        }
        let end = line == format!("END LIST VAR {ups}\n").as_bytes()
            || line == format!("END LIST VAR {ups}\r\n").as_bytes();
        let error = line.starts_with(b"ERR ");
        response.extend(line);
        if error {
            return Err(MetricsError::Protocol);
        }
        if end {
            let text = std::str::from_utf8(&response).map_err(|_| MetricsError::Protocol)?;
            return parse(text, ups);
        }
    }
    Err(MetricsError::Protocol)
}

// Reset the timeout before each actual socket read, so a slowly trickling line
// cannot extend the whole-query deadline indefinitely.
fn read_line_until(
    reader: &mut BufReader<TcpStream>,
    deadline: Instant,
) -> Result<Vec<u8>, MetricsError> {
    let mut line = Vec::new();
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(MetricsError::Deadline)?;
        reader
            .get_ref()
            .set_read_timeout(Some(remaining))
            .map_err(MetricsError::Query)?;
        let bytes = reader.fill_buf().map_err(MetricsError::Query)?;
        if bytes.is_empty() {
            return Err(MetricsError::Protocol);
        }
        let length = bytes
            .iter()
            .position(|b| *b == b'\n')
            .map_or(bytes.len(), |index| index + 1);
        if line.len() + length > MAX_LINE {
            return Err(MetricsError::Protocol);
        }
        line.extend_from_slice(&bytes[..length]);
        reader.consume(length);
        if line.ends_with(b"\n") {
            return Ok(line);
        }
    }
}

/// Accept only a complete, matching LIST VAR frame and a finite metric inventory.
pub fn parse(response: &str, ups: &str) -> Result<Snapshot, MetricsError> {
    validate_name(ups)?;
    if response.len() > MAX_RESPONSE {
        return Err(MetricsError::Protocol);
    }
    let mut lines = response.lines();
    if lines.next() != Some(format!("BEGIN LIST VAR {ups}").as_str()) {
        return Err(MetricsError::Protocol);
    }
    let mut values = BTreeMap::new();
    let mut status = None;
    let mut ended = false;
    let prefix = format!("VAR {ups} ");
    for line in lines.by_ref() {
        if line == format!("END LIST VAR {ups}") {
            ended = true;
            break;
        }
        let body = line.strip_prefix(&prefix).ok_or(MetricsError::Protocol)?;
        let (key, quoted) = body.split_once(' ').ok_or(MetricsError::Protocol)?;
        let value = unquote(quoted)?;
        if key == "ups.status" {
            if status.is_some() {
                return Err(MetricsError::Protocol);
            }
            let flags = value
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>();
            if flags.is_empty()
                || flags
                    .iter()
                    .any(|flag| !flag.bytes().all(|b| b.is_ascii_uppercase()))
            {
                return Err(MetricsError::Protocol);
            }
            status = Some(flags);
        } else if VARIABLES.contains(&key) {
            let number: f64 = value.parse().map_err(|_| MetricsError::Protocol)?;
            if !number.is_finite() || values.insert(key.to_string(), number).is_some() {
                return Err(MetricsError::Protocol);
            }
        }
    }
    if !ended || lines.next().is_some() {
        return Err(MetricsError::Protocol);
    }
    Ok(Snapshot {
        values,
        status: status.ok_or(MetricsError::Protocol)?,
    })
}

fn unquote(input: &str) -> Result<String, MetricsError> {
    let body = input
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .ok_or(MetricsError::Protocol)?;
    let mut result = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(escaped @ ('\\' | '"')) => result.push(escaped),
                _ => return Err(MetricsError::Protocol),
            },
            '"' => return Err(MetricsError::Protocol),
            c if c.is_control() => return Err(MetricsError::Protocol),
            c => result.push(c),
        }
    }
    Ok(result)
}

pub fn render(ups: &str, snapshot: Option<&Snapshot>) -> Result<Vec<String>, MetricsError> {
    validate_name(ups)?;
    let tags = format!("service:nut,ups:{ups}");
    let Some(snapshot) = snapshot else {
        return Ok(vec![format!("_sc|nut.communication|2|#{tags}")]);
    };
    let has = |flag: &str| snapshot.status.iter().any(|s| s == flag);
    let mut packets = snapshot
        .values
        .iter()
        .filter(|(key, _)| VARIABLES.contains(&key.as_str()))
        .map(|(key, value)| format!("nut.{key}:{value}|g|#{tags}"))
        .collect::<Vec<_>>();
    for (flag, name) in FLAGS {
        packets.push(format!(
            "nut.ups.status.{name}:{}|g|#{tags}",
            u8::from(has(flag))
        ));
    }
    let power = if has("LB") || has("FSD") || has("OFF") || has("OVER") {
        2
    } else if has("OB") || has("RB") {
        1
    } else if has("OL") {
        0
    } else {
        3
    };
    packets.push(format!(
        "_sc|nut.communication|{}|#{tags}",
        if has("NOCOMM") { 2 } else { 0 }
    ));
    packets.push(format!("_sc|nut.power|{power}|#{tags}"));
    Ok(packets)
}

/// One datagram per metric/check; sending confirms local acceptance, not ingestion.
pub fn submit(socket: &Path, packets: &[String]) -> Result<(), MetricsError> {
    let client = UnixDatagram::unbound().map_err(MetricsError::Submit)?;
    client.connect(socket).map_err(MetricsError::Submit)?;
    let deadline = Instant::now() + Duration::from_secs(2);
    for packet in packets {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| {
                MetricsError::Submit(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "submission deadline exceeded",
                ))
            })?;
        client
            .set_write_timeout(Some(remaining))
            .map_err(MetricsError::Submit)?;
        let sent = client
            .send(packet.as_bytes())
            .map_err(MetricsError::Submit)?;
        if sent != packet.len() {
            return Err(MetricsError::Submit(std::io::Error::other(
                "short datagram",
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "metrics/tests.rs"]
mod tests;
