use clap::{Parser, Subcommand};

use iputil::cidr::Cidr;
use iputil::render;
use iputil::split;

#[derive(Parser)]
#[command(
    name = "iputil",
    about = "Pure-Rust CIDR/subnet calculator — network/broadcast/host-range and subnet splitting, no ipcalc binary required"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show network/broadcast/netmask/host-range/host-counts for a CIDR
    Info { cidr: String },
    /// Split a CIDR into equal-sized subnets, by count or by target prefix
    Split {
        cidr: String,
        /// Split into exactly this many equal subnets (must be a power of two)
        #[arg(long, conflicts_with = "prefix")]
        count: Option<u64>,
        /// Split into subnets of this new (longer) prefix length
        #[arg(long, conflicts_with = "count")]
        prefix: Option<u8>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Info { cidr } => {
            let c = Cidr::parse(&cidr).map_err(|e| anyhow::anyhow!(e))?;
            println!("{}", render::render_info(&c));
        }
        Command::Split {
            cidr,
            count,
            prefix,
        } => {
            let c = Cidr::parse(&cidr).map_err(|e| anyhow::anyhow!(e))?;
            let subnets = match (count, prefix) {
                (Some(n), None) => split::split_by_count(&c, n).map_err(|e| anyhow::anyhow!(e))?,
                (None, Some(p)) => split::split_by_prefix(&c, p).map_err(|e| anyhow::anyhow!(e))?,
                (None, None) => anyhow::bail!("specify one of --count or --prefix"),
                (Some(_), Some(_)) => {
                    unreachable!("clap enforces --count/--prefix are mutually exclusive")
                }
            };
            println!("{}", render::render_split(&subnets));
        }
    }
    Ok(())
}
