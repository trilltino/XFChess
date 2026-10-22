use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;

use super::{BASE_RPC_URL, ER_RPC_URL, RPC_RETRY_COUNT, RPC_RETRY_DELAY_MS};

pub fn get_blockhash_for_accounts(
    rpc: &RpcClient,
    accounts: &[Pubkey],
) -> solana_client::client_error::Result<solana_sdk::hash::Hash> {
    use solana_client::rpc_request::RpcRequest;

    #[derive(serde::Deserialize)]
    struct BlockhashForAccountsResponse {
        blockhash: String,
    }

    let addrs: Vec<String> = accounts.iter().map(|p| p.to_string()).collect();
    let params = serde_json::json!([addrs]);
    let resp: BlockhashForAccountsResponse = rpc.send(
        RpcRequest::Custom {
            method: "getBlockhashForAccounts",
        },
        params,
    )?;
    resp.blockhash.parse().map_err(|error| {
        solana_client::client_error::ClientError::from(
            solana_client::client_error::ClientErrorKind::Custom(format!(
                "router returned an invalid blockhash: {error}"
            )),
        )
    })
}

pub fn base_client() -> RpcClient {
    RpcClient::new_with_commitment(
        std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| BASE_RPC_URL.into()),
        CommitmentConfig::confirmed(),
    )
}

pub fn er_client() -> RpcClient {
    RpcClient::new_with_commitment(
        std::env::var("ER_RPC_URL").unwrap_or_else(|_| ER_RPC_URL.into()),
        CommitmentConfig::confirmed(),
    )
}

pub async fn with_retry<F, T>(mut f: F) -> Result<T, anyhow::Error>
where
    F: FnMut() -> Result<T, solana_client::client_error::ClientError>,
{
    let mut delay = RPC_RETRY_DELAY_MS;
    for attempt in 0..RPC_RETRY_COUNT {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                eprintln!("   RPC attempt {} failed: {}", attempt + 1, e);
                if let solana_client::client_error::ClientErrorKind::RpcError(
                    solana_client::rpc_request::RpcError::RpcResponseError { data, .. },
                ) = &*e.kind
                {
                    eprintln!("   RPC ERROR DATA: {:?}", data);
                    use solana_client::rpc_request::RpcResponseErrorData;
                    match data {
                        RpcResponseErrorData::SendTransactionPreflightFailure(result) => {
                            if let Some(logs) = &result.logs {
                                eprintln!("   FULL SIMULATION LOGS ({} entries):", logs.len());
                                for log in logs {
                                    eprintln!("     {}", log);
                                }
                            }
                        }
                        _ => {
                            eprintln!("   OTHER ERROR DATA TYPE");
                        }
                    }
                }
                if attempt + 1 < RPC_RETRY_COUNT {
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                    delay *= 2;
                }
            }
        }
    }
    Err(anyhow::anyhow!("Max retries exceeded"))
}
