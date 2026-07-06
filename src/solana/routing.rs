//! Client-side Solana routing boundary for base RPC and Magic Router.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TxRoute {
    Base,
    MagicRouter,
}

pub fn magic_router_url(er_rpc_url: &str) -> String {
    std::env::var("MAGIC_ROUTER_RPC_URL")
        .or_else(|_| std::env::var("MAGIC_ROUTER_URL"))
        .unwrap_or_else(|_| er_rpc_url.to_string())
}

pub fn route_for_game_write(is_delegated: bool) -> TxRoute {
    if is_delegated {
        TxRoute::MagicRouter
    } else {
        TxRoute::Base
    }
}
