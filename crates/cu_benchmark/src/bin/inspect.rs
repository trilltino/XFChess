use clap::Parser;
use cu_benchmark::chain::BASE_RPC_URL;
use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;

#[derive(Parser)]
#[command(about = "Inspect account balances, owners, sizes and rent without a keypair")]
struct Cli {
    #[arg(long, env = "SOLANA_RPC_URL", default_value = BASE_RPC_URL)]
    rpc_url: String,
    #[arg(required = true)]
    accounts: Vec<Pubkey>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let rpc = RpcClient::new_with_commitment(cli.rpc_url, CommitmentConfig::confirmed());
    for address in cli.accounts {
        let account = rpc.get_account(&address)?;
        let rent = rpc.get_minimum_balance_for_rent_exemption(account.data.len())?;
        println!(
            "{address}: owner={} bytes={} balance={} rent_minimum={}",
            account.owner,
            account.data.len(),
            account.lamports,
            rent
        );
    }
    Ok(())
}
