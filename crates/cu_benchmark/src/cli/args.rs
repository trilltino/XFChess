use clap::{Parser, ValueEnum};
use solana_sdk::pubkey::Pubkey;

#[derive(Parser, Debug, Clone)]
#[command(name = "cu_benchmark")]
#[command(about = "Estimate game costs or measure XFChess devnet transactions")]
#[command(version)]
pub(super) struct Cli {
    #[arg(long, conflicts_with_all = ["estimate", "live", "init"])]
    pub(super) scenario: Option<std::path::PathBuf>,

    #[arg(long, requires = "scenario", conflicts_with = "observations")]
    pub(super) execute: bool,

    #[arg(long, requires = "execute")]
    pub(super) allow_transactions: bool,

    #[arg(long, requires = "scenario")]
    pub(super) observations: Option<std::path::PathBuf>,

    #[arg(long, conflicts_with_all = ["live", "init"])]
    pub(super) estimate: Option<std::path::PathBuf>,

    #[arg(long, requires = "estimate")]
    pub(super) quote_rent: bool,

    #[arg(long, requires = "estimate", value_parser = clap::value_parser!(u16).range(1..=10_000))]
    pub(super) priority_percentile: Option<u16>,

    #[arg(long, requires = "priority_percentile")]
    pub(super) writable_account: Vec<Pubkey>,

    #[arg(long, conflicts_with = "init")]
    pub(super) live: bool,

    #[arg(short, long, value_enum, default_value = "1v1")]
    pub(super) mode: TestMode,

    #[arg(short, long, default_value = "16")]
    pub(super) size: u16,

    #[arg(long)]
    pub(super) all: bool,

    #[arg(long)]
    pub(super) skip_funding: bool,

    #[arg(long)]
    pub(super) skip_reclaim: bool,

    #[arg(long)]
    pub(super) export: Option<String>,

    #[arg(long)]
    pub(super) init: bool,
}

#[derive(ValueEnum, Clone, Debug)]
pub(super) enum TestMode {
    #[value(name = "1v1")]
    OneVOne,
    GlobalSession,
    Swiss,
    SingleElim,
    All,
    CrankDrill,
    RecoveryDrill,
}
