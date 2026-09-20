use crate::cidr::Cidr;

/// Split `c` into equal-sized subnets, each with `new_prefix` as its prefix
/// length. `new_prefix` must be at least as long as `c`'s own prefix (you
/// can only make a block *smaller*, splitting it into more pieces) and no
/// longer than the address family's width.
pub fn split_by_prefix(c: &Cidr, new_prefix: u8) -> Result<Vec<Cidr>, String> {
    if new_prefix < c.prefix() {
        return Err(format!(
            "new prefix /{new_prefix} is shorter than /{}; splitting only widens the prefix",
            c.prefix()
        ));
    }
    if new_prefix > c.width() {
        return Err(format!(
            "/{new_prefix} exceeds /{}, the max for this address family",
            c.width()
        ));
    }
    let extra_bits = new_prefix - c.prefix();
    if extra_bits >= 64 {
        return Err(format!(
            "splitting into 2^{extra_bits} subnets is not supported (too many to enumerate)"
        ));
    }
    let count: u128 = 1u128 << extra_bits;
    let block_shift = c.width() - new_prefix;
    // block_shift can be up to 128 only when count == 1 (extra_bits == 0),
    // in which case the block_size value below is never actually used as a
    // per-iteration multiplier for anything past i=0, so 0 is a safe stand-in
    // that avoids a shift-by-128 overflow.
    let block_size: u128 = if block_shift >= 128 {
        0
    } else {
        1u128 << block_shift
    };
    let base = c.network_bits();

    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let addr = base + i * block_size;
        out.push(Cidr::from_raw(addr, new_prefix, c.width()));
    }
    Ok(out)
}

/// Split `c` into exactly `n` equal-sized subnets. `n` must be a power of
/// two — CIDR splitting only ever produces power-of-two subnet counts, so
/// asking for e.g. 3 is rejected with a suggestion (4) rather than silently
/// rounding.
pub fn split_by_count(c: &Cidr, n: u64) -> Result<Vec<Cidr>, String> {
    if n == 0 {
        return Err("cannot split into 0 subnets".to_string());
    }
    if !n.is_power_of_two() {
        let next = n.next_power_of_two();
        return Err(format!(
            "{n} is not a power of two; CIDR splitting only produces power-of-two subnet counts (try {next})"
        ));
    }
    let extra_bits = n.trailing_zeros() as u8;
    split_by_prefix(c, c.prefix() + extra_bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_a_slash_24_into_four_slash_26s() {
        let c = Cidr::parse("10.0.0.0/24").unwrap();
        let subnets = split_by_prefix(&c, 26).unwrap();
        let rendered: Vec<String> = subnets.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            rendered,
            vec![
                "10.0.0.0/26",
                "10.0.0.64/26",
                "10.0.0.128/26",
                "10.0.0.192/26"
            ]
        );
    }

    #[test]
    fn split_by_count_of_four_matches_split_by_prefix_26() {
        let c = Cidr::parse("10.0.0.0/24").unwrap();
        let by_count = split_by_count(&c, 4).unwrap();
        let by_prefix = split_by_prefix(&c, 26).unwrap();
        assert_eq!(by_count, by_prefix);
    }

    #[test]
    fn split_by_count_two_bisects_the_block() {
        let c = Cidr::parse("192.168.0.0/24").unwrap();
        let subnets = split_by_count(&c, 2).unwrap();
        let rendered: Vec<String> = subnets.iter().map(|s| s.to_string()).collect();
        assert_eq!(rendered, vec!["192.168.0.0/25", "192.168.0.128/25"]);
    }

    #[test]
    fn non_power_of_two_count_is_rejected_with_a_suggestion() {
        let c = Cidr::parse("10.0.0.0/24").unwrap();
        let err = split_by_count(&c, 3).unwrap_err();
        assert!(err.contains("not a power of two"), "{err}");
        assert!(err.contains('4'), "{err}");
    }

    #[test]
    fn zero_subnets_is_rejected() {
        let c = Cidr::parse("10.0.0.0/24").unwrap();
        assert!(split_by_count(&c, 0).is_err());
    }

    #[test]
    fn splitting_into_a_shorter_prefix_is_rejected() {
        let c = Cidr::parse("10.0.0.0/24").unwrap();
        let err = split_by_prefix(&c, 23).unwrap_err();
        assert!(err.contains("shorter"), "{err}");
    }

    #[test]
    fn splitting_beyond_the_address_width_is_rejected() {
        let c = Cidr::parse("10.0.0.0/24").unwrap();
        let err = split_by_prefix(&c, 33).unwrap_err();
        assert!(err.contains("exceeds"), "{err}");
    }

    #[test]
    fn splitting_into_the_same_prefix_returns_the_block_unchanged() {
        let c = Cidr::parse("10.0.0.0/24").unwrap();
        let subnets = split_by_prefix(&c, 24).unwrap();
        assert_eq!(subnets, vec![c]);
    }

    #[test]
    fn split_covers_the_whole_original_block_with_no_gaps_or_overlap() {
        let c = Cidr::parse("10.0.0.0/22").unwrap();
        let subnets = split_by_prefix(&c, 24).unwrap();
        assert_eq!(subnets.len(), 4);
        // Each subnet's network address should be exactly one /24 block
        // (256 addresses) past the previous one, and the last one's highest
        // address should land exactly on the original block's highest address.
        let starts: Vec<String> = subnets.iter().map(|s| s.network().to_string()).collect();
        assert_eq!(starts, vec!["10.0.0.0", "10.0.1.0", "10.0.2.0", "10.0.3.0"]);
        assert_eq!(subnets.last().unwrap().highest(), c.highest());
    }

    #[test]
    fn splits_an_ipv6_block_correctly() {
        let c = Cidr::parse("2001:db8::/32").unwrap();
        let subnets = split_by_prefix(&c, 34).unwrap();
        let rendered: Vec<String> = subnets.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            rendered,
            vec![
                "2001:db8::/34",
                "2001:db8:4000::/34",
                "2001:db8:8000::/34",
                "2001:db8:c000::/34",
            ]
        );
    }

    #[test]
    fn splitting_a_slash_0_into_itself_does_not_panic() {
        let c = Cidr::parse("::/0").unwrap();
        let subnets = split_by_prefix(&c, 0).unwrap();
        assert_eq!(subnets, vec![c]);
    }
}
