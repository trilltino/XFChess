//! TURN relay for symmetric NAT traversal
//! Falls back when iroh hole-punching fails (RFC 5766)

use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tracing::{info, warn};

/// TURN relay configuration
#[derive(Debug, Clone)]
pub struct TurnRelayConfig {
    pub server_addr: SocketAddr,
    pub username: String,
    pub password: String,
    pub realm: String,
}

/// NAT type detection result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NatType {
    /// Direct connection possible (no NAT or full-cone)
    OpenOrFullCone,
    /// Restricted cone NAT - may need STUN but not TURN
    RestrictedCone,
    /// Symmetric NAT - requires TURN relay
    Symmetric,
    /// Unknown - need to test
    Unknown,
}

/// TURN relay client
pub struct TurnRelayClient {
    socket: UdpSocket,
    config: TurnRelayConfig,
    relayed_addr: Option<SocketAddr>,
    allocation_lifetime: Duration,
    last_refresh: std::time::Instant,
}

/// Errors that can occur with TURN relay
#[derive(Debug, thiserror::Error)]
pub enum TurnError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TURN allocation failed")]
    AllocationFailed,
    #[error("TURN refresh failed")]
    RefreshFailed,
    #[error("Invalid TURN response")]
    InvalidResponse,
    #[error("Relay not allocated")]
    NotAllocated,
}

impl TurnRelayClient {
    /// Connect to TURN server and create allocation
    pub async fn connect(config: TurnRelayConfig) -> Result<Self, TurnError> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        
        let mut client = Self {
            socket,
            config,
            relayed_addr: None,
            allocation_lifetime: Duration::from_secs(600),
            last_refresh: std::time::Instant::now(),
        };

        client.allocate().await?;
        info!("[turn-relay] Connected to TURN server, relay established");
        
        Ok(client)
    }

    /// Send data through TURN relay to a peer
    pub async fn send_to(&self, data: &[u8], peer: SocketAddr) -> Result<(), TurnError> {
        if self.relayed_addr.is_none() {
            return Err(TurnError::NotAllocated);
        }

        // Send via TURN Send Indication (simplified)
        // In full implementation, this would wrap data in TURN ChannelData or Send Indication
        let _ = peer; // Would use peer address in actual TURN framing
        self.socket.send_to(data, self.config.server_addr).await?;
        
        Ok(())
    }

    /// Receive data from TURN relay
    pub async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), TurnError> {
        let (len, addr) = self.socket.recv_from(buf).await?;
        Ok((len, addr))
    }

    /// Refresh the TURN allocation (call periodically)
    pub async fn refresh(&mut self) -> Result<(), TurnError> {
        if self.last_refresh.elapsed() < self.allocation_lifetime / 2 {
            return Ok(()); // No need to refresh yet
        }

        // In full implementation, send TURN Refresh request
        self.last_refresh = std::time::Instant::now();
        Ok(())
    }

    /// Get the relayed address (what peers should use to reach us)
    pub fn relayed_addr(&self) -> Option<SocketAddr> {
        self.relayed_addr
    }

    /// Close the TURN allocation gracefully
    pub async fn close(&mut self) -> Result<(), TurnError> {
        // In full implementation, send TURN Deallocate
        self.relayed_addr = None;
        Ok(())
    }

    /// Perform TURN allocation
    async fn allocate(&mut self) -> Result<(), TurnError> {
        // Simplified allocation - full RFC 5766 implementation would:
        // 1. Send Allocate request with authentication
        // 2. Handle 401 Unauthorized with nonce
        // 3. Re-send with proper credentials
        // 4. Receive success with relayed address
        
        // For now, simulate successful allocation
        // In production, use a proper STUN/TURN library like `stun` or `turn` crate
        
        self.relayed_addr = Some(self.config.server_addr);
        warn!("[turn-relay] Using simplified TURN allocation - full RFC 5766 not implemented");
        
        Ok(())
    }
}

/// Detect NAT type by attempting various connection strategies
pub async fn detect_nat_type(stun_server: Option<SocketAddr>) -> NatType {
    // Try direct connection first
    if can_connect_direct().await {
        return NatType::OpenOrFullCone;
    }

    // Try STUN if server provided
    if let Some(stun) = stun_server {
        if can_stun_punch(&stun).await {
            return NatType::RestrictedCone;
        }
    }

    // Assume symmetric NAT if other methods fail
    NatType::Symmetric
}

/// Check if direct connection is possible
async fn can_connect_direct() -> bool {
    // Try to bind and see if we get a public address
    match UdpSocket::bind("0.0.0.0:0").await {
        Ok(socket) => {
            // Try to get our public address via a simple echo
            match socket.local_addr() {
                Ok(addr) => {
                    // Check if it's a public IP
                    let ip = addr.ip();
                    let is_private = match ip {
                        std::net::IpAddr::V4(ipv4) => ipv4.is_private(),
                        std::net::IpAddr::V6(_) => false, // Simplified since IpAddr::is_global is nightly
                    };
                    !ip.is_loopback() && !is_private
                }
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

/// Check if STUN hole-punching works
async fn can_stun_punch(_stun_server: &SocketAddr) -> bool {
    // Simplified - full implementation would:
    // 1. Send STUN Binding Request
    // 2. Check mapped address
    // 3. Verify if it's consistent across multiple requests
    
    // For now, return false to prefer TURN
    false
}

/// Check if we're behind symmetric NAT
pub async fn is_symmetric_nat() -> bool {
    matches!(detect_nat_type(None).await, NatType::Symmetric)
}

/// Connection with automatic TURN fallback
pub async fn connect_with_fallback(
    target_addr: SocketAddr,
    turn_config: Option<TurnRelayConfig>,
) -> Result<TurnConnection, TurnError> {
    // Try direct connection first
    match try_direct_connect(target_addr).await {
        Ok(conn) => return Ok(conn),
        Err(e) => {
            warn!("[turn-relay] Direct connection failed: {}", e);
        }
    }

    // Fall back to TURN if available
    if let Some(config) = turn_config {
        info!("[turn-relay] Attempting TURN relay fallback");
        let relay = TurnRelayClient::connect(config).await?;
        Ok(TurnConnection::Relay(relay))
    } else {
        Err(TurnError::AllocationFailed)
    }
}

/// Direct connection result
async fn try_direct_connect(_target: SocketAddr) -> Result<TurnConnection, TurnError> {
    // Simplified - would attempt P2P connection
    Err(TurnError::AllocationFailed)
}

/// Connection type - direct or relayed
pub enum TurnConnection {
    Direct(UdpSocket),
    Relay(TurnRelayClient),
}

impl TurnConnection {
    /// Send data through the connection
    pub async fn send(&self, data: &[u8], addr: SocketAddr) -> Result<(), TurnError> {
        match self {
            TurnConnection::Direct(socket) => {
                socket.send_to(data, addr).await?;
                Ok(())
            }
            TurnConnection::Relay(relay) => {
                relay.send_to(data, addr).await
            }
        }
    }

    /// Receive data
    pub async fn recv(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), TurnError> {
        match self {
            TurnConnection::Direct(socket) => {
                let (len, addr) = socket.recv_from(buf).await?;
                Ok((len, addr))
            }
            TurnConnection::Relay(relay) => {
                relay.recv_from(buf).await
            }
        }
    }
}

/// Default TURN server configuration for production
pub fn default_turn_config() -> Option<TurnRelayConfig> {
    // Read from environment or config file
    let server = std::env::var("TURN_SERVER").ok()?;
    let username = std::env::var("TURN_USERNAME").ok()?;
    let password = std::env::var("TURN_PASSWORD").ok()?;
    
    let addr = server.parse().ok()?;
    
    Some(TurnRelayConfig {
        server_addr: addr,
        username,
        password,
        realm: "xfchess".to_string(),
    })
}
