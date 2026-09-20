# iputil

A pure-Rust CIDR/subnet calculator: network address, broadcast address,
usable host range, host counts, and equal-size subnet splitting — the
things `ipcalc` (a small C program, not always preinstalled, and its
argument syntax differs across the Fedora/Debian/BSD forks) does, without
shelling out to it or vendoring a C dependency. Both IPv4 and IPv6 are
handled through the same code path.

## Usage

```bash
iputil info 10.0.0.0/24
```

```
Network:      10.0.0.0/24
Netmask:      255.255.255.0
Wildcard:     0.0.0.255
Broadcast:    10.0.0.255
Host range:   10.0.0.1 - 10.0.0.254
Total addrs:  256
Usable hosts: 254
```

```bash
iputil split 10.0.0.0/24 --prefix 26      # split into subnets of a given prefix length
iputil split 10.0.0.0/24 --count 4        # split into exactly N equal subnets (N must be a power of two)
```

```
$ iputil split 10.0.0.0/24 --count 4
10.0.0.0/26
10.0.0.64/26
10.0.0.128/26
10.0.0.192/26
```

`--count` and `--prefix` are mutually exclusive — pick whichever framing
fits the problem you have ("I need 4 subnets" vs. "I need /26s").
Asking for a non-power-of-two count (`--count 3`) fails with a concrete
suggestion instead of silently rounding:

```
$ iputil split 10.0.0.0/24 --count 3
Error: 3 is not a power of two; CIDR splitting only produces power-of-two subnet counts (try 4)
```

IPv6 works the same way:

```
$ iputil info 2001:db8::/64
Network:      2001:db8::/64
Netmask:      ffff:ffff:ffff:ffff::
Wildcard:     ::ffff:ffff:ffff:ffff
Highest:      2001:db8::ffff:ffff:ffff:ffff
Host range:   2001:db8::1 - 2001:db8::ffff:ffff:ffff:fffe
Total addrs:  18446744073709551616
Usable hosts: 18446744073709551614
```

(IPv6 has no broadcast address, so `info` labels that line `Highest:`
instead of `Broadcast:` when the input is IPv6.)

## RFC 3021 edge cases

A `/31` has no network/broadcast reservation — both addresses are usable,
by design, for point-to-point links:

```
$ iputil info 10.0.0.0/31
Network:      10.0.0.0/31
...
Host range:   10.0.0.0 - 10.0.0.1
Total addrs:  2
Usable hosts: 2
```

A `/32` (or IPv6 `/128`) is a single host — network, broadcast, and the
one usable address are all the same address. `iputil` gets both of these
right rather than falling into the generic "usable = total - 2" formula,
which would wrongly report 0 or a negative count.

## Status: built and verified against hand-computed CIDR arithmetic

- **30 unit tests** (`cargo test --lib`): CIDR parsing and its error paths
  (missing `/prefix`, invalid IP text, invalid prefix text, prefix beyond
  `/32`, prefix beyond `/128`); a `/24` and a `/16` checked field-by-field
  against hand-computed values (network, netmask, wildcard, broadcast,
  host range, total/usable counts); the RFC 3021 `/31` case (2 usable
  addresses, no reserved network/broadcast); the `/32` single-host case;
  `/0` covering the entire IPv4 address space without overflow; three
  IPv6 cases (`/64`, `/127`, `/128`) including one, `::/0`, that would
  panic on a naive `1u128 << 128` if the shift-overflow guard were
  missing — locked down with its own regression test rather than trusted
  to just work; a non-network-aligned input address (`10.0.0.5/24`)
  correctly normalizing to its network address on display; and the
  splitting logic — a `/24` split into four `/26`s by explicit prefix and
  by `--count 4` producing identical results, a `/24` bisected into two
  `/25`s, a non-power-of-two count rejected with a concrete suggested
  count, splitting into a *shorter* prefix rejected (splitting only
  narrows), splitting beyond the address width rejected, splitting into
  the same prefix returning the block unchanged, an IPv6 `/32` split into
  four `/34`s, and a `/22` split into four `/24`s checked to have no gaps
  or overlap — each subnet's network address exactly one block past the
  last, and the final subnet's highest address landing exactly on the
  original block's highest address.
- **Live-run against the real compiled binary**, not just the test
  suite: `iputil info` against a `/24`, a `/31`, a `/32`, and an IPv6
  `/64`, with every field in the output checked by hand against the
  values above; `iputil split` by both `--prefix` and `--count` checked
  to produce identical output for the same request; the non-power-of-two
  rejection and the missing-`/prefix` parse error both confirmed to exit
  nonzero with a specific, readable message rather than a stack trace or
  silent wrong answer.

**Not done / deliberately deferred**: no "does CIDR A contain address/CIDR
B" membership check (a reasonable companion command, just not one of the
two this tool actually needed to do first); no aggregation/summarization
(the reverse of splitting — given a list of CIDRs, find the smallest
covering supernet); splitting into more than 2^63 subnets is rejected
outright rather than attempted, since generating and printing that many
lines was never going to be a real use case; no `--json` output mode
(plain text only, matching the CLI conventions of the tools it's meant to
sit next to in scripts).
