//! Transaction routing boundary for base RPC, ER RPC, and Magic Router.

use super::rpc::make_rpc;
use solana_client::rpc_client::RpcClient;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxRoute {
    Base,
    MagicRouter,
}

pub fn magic_router_url(er_rpc_url: &str) -> String {
    std::env::var("MAGIC_ROUTER_RPC_URL")
        .or_else(|_| std::env::var("MAGIC_ROUTER_URL"))
        .unwrap_or_else(|_| er_rpc_url.to_string())
}

pub fn routed_rpc(route: TxRoute, base_rpc_url: &str, er_rpc_url: &str) -> RpcClient {
    match route {
        TxRoute::Base => make_rpc(base_rpc_url),
        TxRoute::MagicRouter => make_rpc(&magic_router_url(er_rpc_url)),
    }
}

pub fn route_for_game_write(is_delegated: bool) -> TxRoute {
    if is_delegated {
        TxRoute::MagicRouter
    } else {
        TxRoute::Base
    }
}
