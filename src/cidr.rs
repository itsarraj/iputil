use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// 2^128, for the one case (IPv6 /0) where the true address count doesn't
/// fit in a u128 (whose max value is 2^128 - 1).
const TWO_POW_128: &str = "340282366920938463463374607431768211456";
/// 2^128 - 2, the usable-host equivalent of the above.
const TWO_POW_128_MINUS_2: &str = "340282366920938463463374607431768211454";

/// A parsed CIDR block: an address plus a prefix length. IPv4 and IPv6 share
/// this one representation — the address is stored zero-extended into a
/// u128 and `width` (32 or 128) records which family it came from, so the
/// bit arithmetic below is written once instead of twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    pub(crate) addr: u128,
    pub(crate) prefix: u8,
    pub(crate) width: u8,
}

impl Cidr {
    pub fn parse(s: &str) -> Result<Self, String> {
        let (ip_part, prefix_part) = s
            .split_once('/')
            .ok_or_else(|| format!("{s:?}: missing \"/prefix\" (expected e.g. 10.0.0.0/24)"))?;
        let ip: IpAddr = ip_part
            .parse()
            .map_err(|_| format!("{ip_part:?}: not a valid IP address"))?;
        let width: u8 = match ip {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        let prefix: u8 = prefix_part
            .parse()
            .map_err(|_| format!("{prefix_part:?}: not a valid prefix length"))?;
        if prefix > width {
            return Err(format!(
                "/{prefix}: prefix exceeds /{width}, the max for {}",
                if width == 32 { "IPv4" } else { "IPv6" }
            ));
        }
        let addr = match ip {
            IpAddr::V4(v4) => v4.to_bits() as u128,
            IpAddr::V6(v6) => v6.to_bits(),
        };
        Ok(Cidr {
            addr,
            prefix,
            width,
        })
    }

    pub(crate) fn from_raw(addr: u128, prefix: u8, width: u8) -> Self {
        Cidr {
            addr,
            prefix,
            width,
        }
    }

    pub fn is_v4(&self) -> bool {
        self.width == 32
    }

    pub fn prefix(&self) -> u8 {
        self.prefix
    }

    pub(crate) fn width(&self) -> u8 {
        self.width
    }

    pub(crate) fn width_mask(&self) -> u128 {
        if self.width == 128 {
            u128::MAX
        } else {
            (1u128 << self.width) - 1
        }
    }

    pub(crate) fn netmask_bits(&self) -> u128 {
        if self.prefix == 0 {
            0
        } else if self.prefix >= self.width {
            self.width_mask()
        } else {
            let ones = (1u128 << self.prefix) - 1;
            ones << (self.width - self.prefix)
        }
    }

    fn addr_from_bits(&self, bits: u128) -> IpAddr {
        if self.is_v4() {
            IpAddr::V4(Ipv4Addr::from_bits(bits as u32))
        } else {
            IpAddr::V6(Ipv6Addr::from_bits(bits))
        }
    }

    pub(crate) fn network_bits(&self) -> u128 {
        self.addr & self.netmask_bits()
    }

    pub(crate) fn highest_bits(&self) -> u128 {
        self.network_bits() | (self.width_mask() & !self.netmask_bits())
    }

    pub fn network(&self) -> IpAddr {
        self.addr_from_bits(self.network_bits())
    }

    pub fn netmask(&self) -> IpAddr {
        self.addr_from_bits(self.netmask_bits())
    }

    pub fn wildcard(&self) -> IpAddr {
        self.addr_from_bits(self.width_mask() & !self.netmask_bits())
    }

    /// Highest address in the block — the broadcast address, for IPv4.
    /// IPv6 has no broadcast concept, but the highest address is still a
    /// meaningful boundary to report.
    pub fn highest(&self) -> IpAddr {
        self.addr_from_bits(self.highest_bits())
    }

    /// Total number of addresses in the block, as a decimal string since
    /// an IPv6 /0 (2^128 addresses) doesn't fit in a u128.
    pub fn total_addresses(&self) -> String {
        let shift = self.width - self.prefix;
        match 1u128.checked_shl(shift as u32) {
            Some(n) => n.to_string(),
            None => TWO_POW_128.to_string(),
        }
    }

    /// Usable host range, per RFC 3021 rules for /31 (both addresses
    /// usable, no network/broadcast reserved) and the single-address /32
    /// (and their IPv6 /127, /128 equivalents).
    pub fn usable_range(&self) -> (IpAddr, IpAddr) {
        let host_bits = self.width - self.prefix;
        match host_bits {
            0 => (
                self.addr_from_bits(self.network_bits()),
                self.addr_from_bits(self.network_bits()),
            ),
            1 => (
                self.addr_from_bits(self.network_bits()),
                self.addr_from_bits(self.highest_bits()),
            ),
            _ => (
                self.addr_from_bits(self.network_bits() + 1),
                self.addr_from_bits(self.highest_bits() - 1),
            ),
        }
    }

    pub fn usable_count(&self) -> String {
        let host_bits = self.width - self.prefix;
        match host_bits {
            0 => "1".to_string(),
            1 => "2".to_string(),
            _ => match 1u128.checked_shl(host_bits as u32) {
                Some(n) => (n - 2).to_string(),
                None => TWO_POW_128_MINUS_2.to_string(),
            },
        }
    }
}

impl fmt::Display for Cidr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.network(), self.prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_plain_v4_cidr() {
        let c = Cidr::parse("10.0.0.0/24").unwrap();
        assert!(c.is_v4());
        assert_eq!(c.prefix(), 24);
    }

    #[test]
    fn rejects_missing_prefix() {
        let err = Cidr::parse("10.0.0.0").unwrap_err();
        assert!(err.contains("/prefix"), "{err}");
    }

    #[test]
    fn rejects_invalid_ip() {
        let err = Cidr::parse("not.an.ip/24").unwrap_err();
        assert!(err.contains("not a valid IP address"), "{err}");
    }

    #[test]
    fn rejects_invalid_prefix_text() {
        let err = Cidr::parse("10.0.0.0/abc").unwrap_err();
        assert!(err.contains("not a valid prefix length"), "{err}");
    }

    #[test]
    fn rejects_prefix_beyond_v4_width() {
        let err = Cidr::parse("10.0.0.0/33").unwrap_err();
        assert!(err.contains("exceeds /32"), "{err}");
    }

    #[test]
    fn rejects_prefix_beyond_v6_width() {
        let err = Cidr::parse("::1/129").unwrap_err();
        assert!(err.contains("exceeds /128"), "{err}");
    }

    // Hand-computed reference case: 10.0.0.0/24.
    #[test]
    fn slash_24_matches_hand_computed_values() {
        let c = Cidr::parse("10.0.0.0/24").unwrap();
        assert_eq!(c.network().to_string(), "10.0.0.0");
        assert_eq!(c.netmask().to_string(), "255.255.255.0");
        assert_eq!(c.wildcard().to_string(), "0.0.0.255");
        assert_eq!(c.highest().to_string(), "10.0.0.255");
        assert_eq!(c.total_addresses(), "256");
        assert_eq!(c.usable_count(), "254");
        let (lo, hi) = c.usable_range();
        assert_eq!(lo.to_string(), "10.0.0.1");
        assert_eq!(hi.to_string(), "10.0.0.254");
    }

    #[test]
    fn a_host_address_normalizes_to_its_network_on_display() {
        // .5 is not network-aligned for a /24; network() must still resolve
        // to .0, and Display should show the normalized form.
        let c = Cidr::parse("10.0.0.5/24").unwrap();
        assert_eq!(c.network().to_string(), "10.0.0.0");
        assert_eq!(c.to_string(), "10.0.0.0/24");
    }

    #[test]
    fn slash_16_matches_hand_computed_values() {
        let c = Cidr::parse("172.16.0.0/16").unwrap();
        assert_eq!(c.netmask().to_string(), "255.255.0.0");
        assert_eq!(c.highest().to_string(), "172.16.255.255");
        assert_eq!(c.total_addresses(), "65536");
        assert_eq!(c.usable_count(), "65534");
    }

    // RFC 3021: /31 has no network/broadcast reservation, both addresses usable.
    #[test]
    fn slash_31_has_two_usable_addresses_per_rfc3021() {
        let c = Cidr::parse("10.0.0.0/31").unwrap();
        assert_eq!(c.network().to_string(), "10.0.0.0");
        assert_eq!(c.highest().to_string(), "10.0.0.1");
        assert_eq!(c.total_addresses(), "2");
        assert_eq!(c.usable_count(), "2");
        let (lo, hi) = c.usable_range();
        assert_eq!(lo.to_string(), "10.0.0.0");
        assert_eq!(hi.to_string(), "10.0.0.1");
    }

    // /32 is a single host: network == highest == the address itself.
    #[test]
    fn slash_32_is_a_single_host() {
        let c = Cidr::parse("10.0.0.7/32").unwrap();
        assert_eq!(c.network().to_string(), "10.0.0.7");
        assert_eq!(c.highest().to_string(), "10.0.0.7");
        assert_eq!(c.total_addresses(), "1");
        assert_eq!(c.usable_count(), "1");
        let (lo, hi) = c.usable_range();
        assert_eq!(lo.to_string(), "10.0.0.7");
        assert_eq!(hi.to_string(), "10.0.0.7");
    }

    #[test]
    fn slash_0_covers_the_entire_v4_space() {
        let c = Cidr::parse("0.0.0.0/0").unwrap();
        assert_eq!(c.network().to_string(), "0.0.0.0");
        assert_eq!(c.highest().to_string(), "255.255.255.255");
        assert_eq!(c.total_addresses(), "4294967296"); // 2^32
    }

    #[test]
    fn ipv6_slash_64_matches_hand_computed_values() {
        let c = Cidr::parse("2001:db8::/64").unwrap();
        assert!(!c.is_v4());
        assert_eq!(c.network().to_string(), "2001:db8::");
        assert_eq!(c.highest().to_string(), "2001:db8::ffff:ffff:ffff:ffff");
        assert_eq!(c.total_addresses(), "18446744073709551616"); // 2^64
    }

    #[test]
    fn ipv6_slash_127_has_two_usable_addresses() {
        let c = Cidr::parse("2001:db8::/127").unwrap();
        assert_eq!(c.usable_count(), "2");
    }

    #[test]
    fn ipv6_slash_128_is_a_single_host() {
        let c = Cidr::parse("::1/128").unwrap();
        assert_eq!(c.network().to_string(), "::1");
        assert_eq!(c.highest().to_string(), "::1");
        assert_eq!(c.usable_count(), "1");
    }

    // The one case where the true count doesn't fit in a u128: must not panic.
    #[test]
    fn ipv6_slash_0_does_not_panic_and_reports_two_pow_128() {
        let c = Cidr::parse("::/0").unwrap();
        assert_eq!(c.total_addresses(), TWO_POW_128);
        assert_eq!(c.usable_count(), TWO_POW_128_MINUS_2);
        assert_eq!(
            c.highest().to_string(),
            "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff"
        );
    }
}
