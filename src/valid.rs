//! Shape validators behind the table value parsers (`dt`, `date`,
//! `time`, `inet`, `cidr`, `macaddr`, `macaddr8`, `uuid`).
//!
//! Crate-private on purpose: these back the table type ids, they are not
//! a user API. Always compiled (pure std) — the Cargo features gate
//! whether the table stage uses them for a type id, not the functions.
//! `inet`, `cidr`, `macaddr` and `macaddr8` follow the PostgreSQL type
//! definitions. Every accept/reject rule is exercised by the unit tests
//! at the bottom of this file.

// always compiled but only called by feature-gated table code — without
// dead_code the unused-validator warnings would depend on the feature set
#![allow(dead_code)]

use std::net::IpAddr;

/// `YYYY-MM-DD`, optionally followed by `T` (or a space) and
/// `hh:mm:ss[.fraction]` with an optional `Z` / `±hh:mm` offset — the
/// shape of the table `dt` type.
///
/// Accepts `2024-01-02`, `2024-01-02T03:04:05Z`,
/// `2024-01-02 03:04:05.123+08:00`. Rejects `2024-1-2` (fields are
/// fixed-width), `2024-01-02X03:04:05` (separator must be `T` or a
/// space), `2024-01-02T03:04` (seconds are required).
pub(crate) fn datetime(s: &str) -> bool {
    let b = s.as_bytes();
    if !date_shape(b) {
        return false;
    }
    if b.len() == 10 {
        return true;
    }
    if b[10] != b'T' && b[10] != b' ' {
        return false;
    }
    let Some(i) = time_shape(b, 11) else {
        return false;
    };
    match &b[i..] {
        [] | [b'Z'] => true,
        [sign, h1, h2, b':', m1, m2] => {
            (*sign == b'+' || *sign == b'-')
                && h1.is_ascii_digit()
                && h2.is_ascii_digit()
                && m1.is_ascii_digit()
                && m2.is_ascii_digit()
        }
        _ => false,
    }
}

/// Exactly `YYYY-MM-DD` — the table `date` type. A full datetime is not
/// a date.
pub(crate) fn date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10 && date_shape(b)
}

/// Exactly `hh:mm:ss[.fraction]` — the table `time` type. Fixed-width,
/// seconds required, no offsets on bare times.
pub(crate) fn time(s: &str) -> bool {
    let b = s.as_bytes();
    time_shape(b, 0) == Some(b.len())
}

/// PostgreSQL `inet`: a host address of either IP family with an
/// optional `/netmask`; bits to the right of the netmask may be set —
/// that is what distinguishes it from [`cidr`]. `10.0.0.1/24` is a valid
/// inet, `10.0.0.1/33` (mask exceeds the family length) is not.
pub(crate) fn inet(s: &str) -> bool {
    let (addr, prefix) = split_mask(s);
    let Ok(ip) = addr.parse::<IpAddr>() else {
        return false;
    };
    match prefix {
        None => true,
        Some(prefix) => parse_prefix(prefix, family_bits(ip)).is_some(),
    }
}

/// PostgreSQL `cidr`: a network of either IP family. The netmask is
/// optional (it defaults to the full address length, so a bare address
/// is a host network) and bits to the right of the netmask must be zero:
/// `10.0.0.0/8` is a network, `10.0.0.1/24` is not (that is an inet).
pub(crate) fn cidr(s: &str) -> bool {
    let (addr, prefix) = split_mask(s);
    let Ok(ip) = addr.parse::<IpAddr>() else {
        return false;
    };
    let bits = family_bits(ip);
    let prefix = match prefix {
        None => bits,
        Some(prefix) => match parse_prefix(prefix, bits) {
            Some(p) => p,
            None => return false,
        },
    };
    host_bits_zero(ip, prefix)
}

/// PostgreSQL `macaddr` input formats — exactly these groupings, one
/// uniform separator:
///
/// - `08:00:2b:01:02:03` / `08-00-2b-01-02-03` (six pairs, `:` or `-`)
/// - `08002b:010203` / `08002b-010203` (two six-digit halves)
/// - `0800.2b01.0203` / `0800-2b01-0203` (three four-digit groups,
///   `.` or `-`)
/// - `08002b010203` (bare)
///
/// Mixed separators or any other grouping are rejected. This is the rule
/// for explicit `macaddr` cells; the `auto` guess uses the stricter
/// [`macaddr_pairs`].
pub(crate) fn macaddr(s: &str) -> bool {
    let Some((sep, groups)) = mac_groups(s) else {
        return false;
    };
    match sep {
        None => groups == [12],
        Some(':' | '-') if groups == [2, 2, 2, 2, 2, 2] || groups == [6, 6] => true,
        Some('.' | '-') if groups == [4, 4, 4] => true,
        _ => false,
    }
}

/// PostgreSQL `macaddr8` (EUI-64) input formats, analogous to
/// [`macaddr`]:
///
/// - eight pairs with `:` or `-`
/// - `08002b:0102030405` (3+5 byte halves) and `08002b01:02030405`
///   (4+4 byte halves), `:` or `-`
/// - four four-digit groups with `.` or `-`
/// - bare 16 hex digits
pub(crate) fn macaddr8(s: &str) -> bool {
    let Some((sep, groups)) = mac_groups(s) else {
        return false;
    };
    match sep {
        None => groups == [16],
        Some(':' | '-')
            if groups == [2, 2, 2, 2, 2, 2, 2, 2] || groups == [6, 10] || groups == [8, 8] =>
        {
            true
        }
        Some('.' | '-') if groups == [4, 4, 4, 4] => true,
        _ => false,
    }
}

/// The conventional six-pair spelling only (`xx:xx:xx:xx:xx:xx`, `:` or
/// `-`). This is what the `auto` guess accepts, so arbitrary hex blobs
/// never auto-convert into MACs.
pub(crate) fn macaddr_pairs(s: &str) -> bool {
    matches!(mac_groups(s), Some((Some(':' | '-'), g)) if g == [2, 2, 2, 2, 2, 2])
}

/// The conventional eight-pair spelling, like [`macaddr_pairs`].
pub(crate) fn macaddr8_pairs(s: &str) -> bool {
    matches!(mac_groups(s), Some((Some(':' | '-'), g)) if g == [2, 2, 2, 2, 2, 2, 2, 2])
}

/// The table `uuid` type: hyphenated `8-4-4-4-12` hex or the bare
/// 32-hex form, case-insensitive. The `auto` guess uses the stricter
/// [`uuid_hyphenated`] so plain hex blobs never auto-convert.
pub(crate) fn uuid(s: &str) -> bool {
    uuid_hyphenated(s) || (s.len() == 32 && s.bytes().all(|c| c.is_ascii_hexdigit()))
}

/// Hyphenated `8-4-4-4-12` hex only — what `auto` accepts as a UUID.
pub(crate) fn uuid_hyphenated(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, &c)| match i {
            8 | 13 | 18 | 23 => c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// Split a MAC-shaped string into its uniform separator (`None` = bare)
/// and hex-group lengths. `None` overall when a non-hex, non-separator
/// character appears or the separators are mixed.
fn mac_groups(s: &str) -> Option<(Option<char>, Vec<usize>)> {
    if s.is_empty() {
        return None;
    }
    let mut sep = None;
    for c in s.chars() {
        if c.is_ascii_hexdigit() {
            continue;
        }
        if !matches!(c, ':' | '-' | '.') {
            return None;
        }
        match sep {
            None => sep = Some(c),
            Some(existing) if existing == c => {}
            Some(_) => return None, // mixed separators
        }
    }
    let groups = match sep {
        Some(c) => s.split(c).map(str::len).collect(),
        None => vec![s.len()],
    };
    Some((sep, groups))
}

/// `YYYY-MM-DD` at the start of `b`.
fn date_shape(b: &[u8]) -> bool {
    let digit = |i: usize| b.get(i).is_some_and(u8::is_ascii_digit);
    b.len() >= 10
        && (0..4).all(digit)
        && b[4] == b'-'
        && (5..7).all(digit)
        && b[7] == b'-'
        && (8..10).all(digit)
}

/// `hh:mm:ss[.fraction]` starting at `i`; returns the index after it.
fn time_shape(b: &[u8], i: usize) -> Option<usize> {
    let digit = |i: usize| b.get(i).is_some_and(u8::is_ascii_digit);
    let ok = digit(i)
        && digit(i + 1)
        && b.get(i + 2) == Some(&b':')
        && digit(i + 3)
        && digit(i + 4)
        && b.get(i + 5) == Some(&b':')
        && digit(i + 6)
        && digit(i + 7);
    if !ok {
        return None;
    }
    let mut end = i + 8;
    if b.get(end) == Some(&b'.') {
        let fraction_start = end + 1;
        end += 1;
        while digit(end) {
            end += 1;
        }
        if end == fraction_start {
            return None;
        }
    }
    Some(end)
}

fn split_mask(s: &str) -> (&str, Option<&str>) {
    match s.split_once('/') {
        Some((addr, prefix)) => (addr, Some(prefix)),
        None => (s, None),
    }
}

fn family_bits(ip: IpAddr) -> u32 {
    match ip {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    }
}

fn parse_prefix(prefix: &str, bits: u32) -> Option<u32> {
    if prefix.is_empty() || !prefix.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    prefix.parse::<u32>().ok().filter(|p| *p <= bits)
}

/// Bits to the right of the netmask are all zero.
fn host_bits_zero(ip: IpAddr, prefix: u32) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let host = if prefix >= 32 { 0 } else { u32::MAX >> prefix };
            u32::from(v4) & host == 0
        }
        IpAddr::V6(v6) => {
            let host = if prefix >= 128 {
                0
            } else {
                u128::MAX >> prefix
            };
            u128::from(v6) & host == 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datetime_shapes() {
        assert!(datetime("2024-01-02"));
        assert!(datetime("2024-01-02T03:04:05"));
        assert!(datetime("2024-01-02T03:04:05Z"));
        assert!(datetime("2024-01-02 03:04:05.123+08:00"));
        assert!(datetime("2024-01-02T03:04:05-07:00"));

        assert!(!datetime("tomorrow"));
        assert!(!datetime("2024-1-2")); // fields are fixed-width
        assert!(!datetime("2024-01-02X03:04:05")); // separator must be T or space
        assert!(!datetime("2024-01-02T03:04")); // seconds required
        assert!(!datetime("2024-01-02T03:04:05.")); // empty fraction
        assert!(!datetime("2024-01-02T03:04:05+8:00")); // offset is hh:mm
    }

    #[test]
    fn date_shapes() {
        assert!(date("2024-01-02"));

        assert!(!date("2024-1-2"));
        assert!(!date("2024-01-02T03:04:05Z")); // that is a datetime
        assert!(!date("20240102"));
    }

    #[test]
    fn time_shapes() {
        assert!(time("03:04:05"));
        assert!(time("03:04:05.5"));

        assert!(!time("3:04:05")); // fixed-width
        assert!(!time("03:04")); // seconds required
        assert!(!time("03:04:05.")); // empty fraction
        assert!(!time("03:04:05+08:00")); // no offsets on bare times
    }

    #[test]
    fn inet_shapes() {
        assert!(inet("10.0.0.1"));
        assert!(inet("10.0.0.1/24")); // host bits below the mask are fine
        assert!(inet("::1"));
        assert!(inet("fe80::1/64"));
        assert!(inet("10.0.0.1/32"));

        assert!(!inet("10.0.0.1/33")); // mask exceeds the family length
        assert!(!inet("10.0.0/24")); // not a full address
        assert!(!inet("10.0.0.1/")); // empty mask
        assert!(!inet("example.com"));
    }

    #[test]
    fn cidr_shapes() {
        assert!(cidr("10.0.0.0/8"));
        assert!(cidr("10.1.2.3")); // = 10.1.2.3/32, a host network
        assert!(cidr("2001:db8::/32"));
        assert!(cidr("::/0"));

        assert!(!cidr("10.0.0.1/24")); // host bits set below the mask
        assert!(!cidr("2001:db8::1/32")); // same, IPv6
        assert!(!cidr("10.0.0.0/33")); // mask exceeds the family length
        assert!(!cidr("10.0.0.0/")); // empty mask
    }

    #[test]
    fn macaddr_postgres_forms() {
        assert!(macaddr("08:00:2b:01:02:03"));
        assert!(macaddr("08-00-2b-01-02-03"));
        assert!(macaddr("08002b:010203"));
        assert!(macaddr("08002b-010203"));
        assert!(macaddr("0800.2b01.0203"));
        assert!(macaddr("0800-2b01-0203"));
        assert!(macaddr("08002b010203"));

        assert!(!macaddr("08:00:2b:01:02")); // five pairs
        assert!(!macaddr("08:00-2b:01:02:03")); // mixed separators
        assert!(!macaddr("0800:2b01:0203")); // 4-digit groups take . or - only
        assert!(!macaddr("08002b.010203")); // 6-digit halves take : or - only
        assert!(!macaddr("08:002b01:0203")); // grouping not in the list
        assert!(!macaddr("08002b01020g")); // not hex
    }

    #[test]
    fn macaddr8_postgres_forms() {
        assert!(macaddr8("08:00:2b:01:02:03:04:05"));
        assert!(macaddr8("08-00-2b-01-02-03-04-05"));
        assert!(macaddr8("08002b:0102030405")); // 3+5 byte halves
        assert!(macaddr8("08002b01:02030405")); // 4+4 byte halves
        assert!(macaddr8("0800.2b01.0203.0405"));
        assert!(macaddr8("0800-2b01-0203-0405"));
        assert!(macaddr8("08002b0102030405"));

        assert!(!macaddr8("08:00:2b:01:02:03")); // that is 6 bytes
        assert!(!macaddr8("08002b0102030405ff")); // 9 bytes
    }

    #[test]
    fn mac_pair_spellings_only() {
        assert!(macaddr_pairs("08:00:2b:01:02:03"));
        assert!(macaddr_pairs("08-00-2b-01-02-03"));
        assert!(macaddr8_pairs("aa:bb:cc:dd:ee:ff:00:11"));

        assert!(!macaddr_pairs("08002b:010203")); // valid macaddr, not pairs
        assert!(!macaddr_pairs("08002b010203")); // bare
        assert!(!macaddr8_pairs("aa:bb:cc:dd:ee:ff")); // 6 bytes
    }

    #[test]
    fn uuid_shapes() {
        assert!(uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(uuid("550E8400-E29B-41D4-A716-446655440000")); // case-insensitive
        assert!(uuid("550e8400e29b41d4a716446655440000")); // bare form, explicit only

        assert!(!uuid("550e8400-e29b-41d4-a716-44665544000")); // too short
        assert!(!uuid("550e8400-e29b-41d4-a716-44665544000g")); // not hex
        assert!(!uuid("550e8400e29b41d4a71644665544000")); // bare but 31 digits

        // the auto guess takes the hyphenated spelling only
        assert!(uuid_hyphenated("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!uuid_hyphenated("550e8400e29b41d4a716446655440000"));
    }
}
