use crate::cidr::Cidr;

pub fn render_info(c: &Cidr) -> String {
    let (lo, hi) = c.usable_range();
    let highest_label = if c.is_v4() { "Broadcast:" } else { "Highest:" };
    let mut lines = vec![
        format!("Network:      {}/{}", c.network(), c.prefix()),
        format!("Netmask:      {}", c.netmask()),
        format!("Wildcard:     {}", c.wildcard()),
        format!("{highest_label:<13} {}", c.highest()),
        format!("Host range:   {lo} - {hi}"),
        format!("Total addrs:  {}", c.total_addresses()),
        format!("Usable hosts: {}", c.usable_count()),
    ];
    lines.retain(|l| !l.is_empty());
    lines.join("\n")
}

pub fn render_split(subnets: &[Cidr]) -> String {
    subnets
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_uses_broadcast_label_for_v4() {
        let c = Cidr::parse("10.0.0.0/24").unwrap();
        let text = render_info(&c);
        assert!(text.contains("Broadcast:"));
        assert!(text.contains("10.0.0.255"));
    }

    #[test]
    fn info_uses_highest_label_for_v6() {
        let c = Cidr::parse("2001:db8::/64").unwrap();
        let text = render_info(&c);
        assert!(text.contains("Highest:"));
        assert!(!text.contains("Broadcast:"));
    }

    #[test]
    fn split_renders_one_cidr_per_line() {
        let c = Cidr::parse("10.0.0.0/24").unwrap();
        let subnets = crate::split::split_by_prefix(&c, 26).unwrap();
        let text = render_split(&subnets);
        assert_eq!(text.lines().count(), 4);
        assert_eq!(text.lines().next().unwrap(), "10.0.0.0/26");
    }
}
