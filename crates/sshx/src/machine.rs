//! Stable, per-machine identity derivation for sshx sessions.
//!
//! By default, sshx asks the server for a *random* session ID every time it
//! starts, which means the shareable URL changes on every restart. For
//! self-hosted, long-lived machines it is often more convenient to have a
//! **fixed** URL per machine that survives restarts.
//!
//! This module derives that fixed identity deterministically from a stable
//! machine fingerprint (the network card MAC address, with fallbacks), so the
//! same machine always produces the same session ID and the same end-to-end
//! encryption key — and therefore the same URL.
//!
//! The derivation is a pure function of the fingerprint, so nothing about the
//! machine identity is ever sent to or stored on the server beyond the session
//! ID itself (exactly like the random flow). The encryption key never leaves
//! the client.

use anyhow::{bail, Result};
use sha2::{Digest, Sha256};
use tracing::debug;

/// Length of the derived session ID (matches the random default of 10).
const SESSION_ID_LEN: usize = 10;

/// Length of the derived encryption key (matches the random default of 14).
const ENCRYPTION_KEY_LEN: usize = 14;

/// Length of the derived write password (matches the random default of 14).
const WRITE_PASSWORD_LEN: usize = 14;

/// Alphanumeric alphabet, matching the character set of `rand_alphanumeric`.
const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// A stable identity derived from the machine fingerprint.
#[derive(Debug, Clone)]
pub struct StableIdentity {
    /// Fixed session ID, sent to the server as the desired session name.
    pub session_id: String,
    /// Fixed end-to-end encryption key (URL fragment). Never sent to server.
    pub encryption_key: String,
    /// Fixed write password, used only in read-only (`--enable-readers`) mode.
    pub write_password: String,
    /// Human-readable description of where the fingerprint came from (logging).
    pub source: String,
}

impl StableIdentity {
    /// Resolve the stable identity for this machine.
    ///
    /// If `override_seed` is `Some` and non-empty (e.g. from `--machine-seed`
    /// or `SSHX_MACHINE_SEED`), it is used verbatim as the fingerprint instead
    /// of auto-detecting one. Otherwise the fingerprint is derived from, in
    /// order of preference: the network MAC address, the system machine-id, or
    /// the hostname.
    pub fn resolve(override_seed: Option<&str>) -> Result<Self> {
        let (fingerprint, source) = machine_fingerprint(override_seed)?;
        debug!(%source, "derived stable machine identity");
        Ok(Self {
            session_id: stable_alphanumeric(
                fingerprint.as_bytes(),
                "sshx/session-id",
                SESSION_ID_LEN,
            ),
            encryption_key: stable_alphanumeric(
                fingerprint.as_bytes(),
                "sshx/encryption-key",
                ENCRYPTION_KEY_LEN,
            ),
            write_password: stable_alphanumeric(
                fingerprint.as_bytes(),
                "sshx/write-password",
                WRITE_PASSWORD_LEN,
            ),
            source,
        })
    }
}

/// Deterministically derive a fixed-length alphanumeric string from arbitrary
/// seed material and a context label.
///
/// This is a small HKDF-style expansion built on SHA-256, using rejection
/// sampling to map bytes onto the 62-character alphanumeric alphabet without
/// modulo bias. It is fully deterministic and stable across versions, so the
/// same `(seed, context, len)` always yields the same output.
pub fn stable_alphanumeric(seed_material: &[u8], context: &str, len: usize) -> String {
    const DOMAIN: &[u8] = b"sshx-stable-alnum-v1";
    // 62 * 4 == 248; reject bytes >= 248 to avoid modulo bias.
    const REJECT_THRESHOLD: u8 = 248;

    let mut out = String::with_capacity(len);
    let mut counter: u32 = 0;
    while out.len() < len {
        let mut hasher = Sha256::new();
        hasher.update(DOMAIN);
        hasher.update((context.len() as u32).to_be_bytes());
        hasher.update(context.as_bytes());
        hasher.update(counter.to_be_bytes());
        hasher.update(seed_material);
        let digest = hasher.finalize();
        for &b in digest.iter() {
            if b < REJECT_THRESHOLD {
                out.push(ALPHABET[(b % 62) as usize] as char);
                if out.len() == len {
                    break;
                }
            }
        }
        counter += 1;
    }
    out
}

/// Compute a stable fingerprint string for this machine, plus a human-readable
/// description of its source.
fn machine_fingerprint(override_seed: Option<&str>) -> Result<(String, String)> {
    if let Some(seed) = override_seed {
        let seed = seed.trim();
        if !seed.is_empty() {
            return Ok((
                format!("override:{seed}"),
                "explicit seed (--machine-seed / SSHX_MACHINE_SEED)".to_string(),
            ));
        }
    }

    if let Some((mac, universal)) = choose_mac() {
        let kind = if universal {
            "hardware"
        } else {
            "locally-administered"
        };
        return Ok((format!("mac:{mac}"), format!("{kind} MAC address {mac}")));
    }

    if let Some(machine_id) = read_machine_id() {
        return Ok((
            format!("machine-id:{machine_id}"),
            "system machine-id".to_string(),
        ));
    }

    if let Some(host) = whoami::fallible::hostname().ok().filter(|h| !h.is_empty()) {
        return Ok((format!("hostname:{host}"), format!("hostname {host}")));
    }

    bail!(
        "could not determine a stable machine identifier (no MAC address, machine-id, or hostname \
         available); pass --machine-seed <value> or run with --ephemeral for a one-off random \
         session"
    )
}

/// Pick a single stable MAC address for this machine.
///
/// Prefers universally-administered (real hardware) addresses over
/// locally-administered/virtual ones, and picks the smallest such address for
/// determinism regardless of interface enumeration order. Returns the address
/// and whether it is universally administered.
fn choose_mac() -> Option<(String, bool)> {
    use mac_address::MacAddressIterator;

    let mut addrs: Vec<[u8; 6]> = MacAddressIterator::new()
        .ok()?
        .map(|m| m.bytes())
        // Skip the all-zero placeholder and any multicast addresses.
        .filter(|b| *b != [0u8; 6] && (b[0] & 0x01 == 0))
        .collect();
    addrs.sort_unstable();
    addrs.dedup();

    if addrs.is_empty() {
        return None;
    }

    // Universally-administered addresses have the second-least-significant bit
    // of the first octet cleared; these are stable hardware addresses.
    if let Some(b) = addrs.iter().find(|b| b[0] & 0x02 == 0) {
        return Some((format_mac(b), true));
    }
    Some((format_mac(&addrs[0]), false))
}

/// Format a MAC address as lowercase, colon-separated hex.
fn format_mac(b: &[u8; 6]) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5]
    )
}

/// Read a persistent system machine-id, if available (Linux/systemd/D-Bus).
fn read_machine_id() -> Option<String> {
    for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Ok(contents) = std::fs::read_to_string(path) {
            let id = contents.trim();
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_correct_charset() {
        let fp = b"mac:aa:bb:cc:dd:ee:ff";
        let a1 = stable_alphanumeric(fp, "sshx/session-id", 10);
        let a2 = stable_alphanumeric(fp, "sshx/session-id", 10);
        assert_eq!(a1, a2, "derivation must be deterministic");
        assert_eq!(a1.len(), 10);
        assert!(a1.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn matches_reference_vectors() {
        // These vectors are pinned against an independent reference
        // implementation; changing them would break existing fixed URLs.
        let fp = b"mac:aa:bb:cc:dd:ee:ff";
        assert_eq!(stable_alphanumeric(fp, "sshx/session-id", 10), "cv0yMdoqDA");
        assert_eq!(
            stable_alphanumeric(fp, "sshx/encryption-key", 14),
            "P2zsXGsYxHXOKe"
        );
        assert_eq!(
            stable_alphanumeric(fp, "sshx/write-password", 14),
            "ViWng66hGuVSgy"
        );
    }

    #[test]
    fn contexts_are_independent() {
        let fp = b"seed";
        let sid = stable_alphanumeric(fp, "sshx/session-id", 14);
        let key = stable_alphanumeric(fp, "sshx/encryption-key", 14);
        let wp = stable_alphanumeric(fp, "sshx/write-password", 14);
        assert_ne!(sid, key);
        assert_ne!(key, wp);
        assert_ne!(sid, wp);
    }

    #[test]
    fn different_seeds_differ() {
        let a = stable_alphanumeric(b"mac:aa:bb:cc:dd:ee:ff", "sshx/session-id", 10);
        let b = stable_alphanumeric(b"mac:11:22:33:44:55:66", "sshx/session-id", 10);
        assert_ne!(a, b);
    }

    #[test]
    fn explicit_seed_is_used() {
        let (fp, _src) = machine_fingerprint(Some("my-fixed-seed")).unwrap();
        assert_eq!(fp, "override:my-fixed-seed");
    }

    #[test]
    fn format_mac_is_lowercase_hex() {
        assert_eq!(
            format_mac(&[0xAA, 0x0B, 0xcc, 0x00, 0xEE, 0xff]),
            "aa:0b:cc:00:ee:ff"
        );
    }
}
