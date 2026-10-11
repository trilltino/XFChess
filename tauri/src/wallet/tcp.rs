use super::popup::{open_wallet_popup, open_wallet_popup_for_signing};
use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Clone)]
struct Bridge {
  app: tauri::AppHandle,
  pending: PendingTx,
  notify: PendingTxNotify,
  wallet: WalletPubkey,
}

pub(crate) fn start_tcp_bridge(
  app: &tauri::AppHandle,
  pending: &PendingTx,
  notify: &PendingTxNotify,
  wallet: &WalletPubkey,
) {
  let bridge = Bridge {
    app: app.clone(),
    pending: pending.clone(),
    notify: notify.clone(),
    wallet: wallet.clone(),
  };
  tauri::async_runtime::spawn(async move {
    // Match the client's fallback scan, reserving the HTTP port first.
    let mut listener = None;
    for offset in (2u16..=11).rev() {
      if let Ok(bound) = TcpListener::bind(std::net::SocketAddr::from((
        [127, 0, 0, 1],
        http_port().saturating_sub(offset),
      )))
      .await
      {
        listener = Some(bound);
        break;
      }
    }
    let Some(listener) = listener else {
      tracing::error!("No wallet TCP port available");
      return;
    };
    let port = listener.local_addr().expect("bound TCP address").port();
    let _ = TCP_PORT.set(port);
    if let Err(error) = std::fs::write(wallet_bridge_port_file(http_port()), port.to_string()) {
      tracing::warn!(%error, "Cannot announce wallet TCP port");
    }
    if super::lifecycle::stopped() {
      super::remove_port_files();
      return;
    }
    while let Ok((stream, _)) = listener.accept().await {
      let bridge = bridge.clone();
      tokio::spawn(async move {
        if let Err(error) = handle(stream, bridge).await {
          tracing::debug!(%error, "Wallet TCP connection ended");
        }
      });
    }
  });
}

async fn read_frame(
  stream: &mut TcpStream,
  prefix: [u8; 4],
) -> std::io::Result<(String, Vec<u8>, &'static str)> {
  let kind = if &prefix == b"MSIG" {
    "message"
  } else {
    "transaction"
  };
  let label_len = if kind == "message" {
    stream.read_u32_le().await?
  } else {
    u32::from_le_bytes(prefix)
  };
  if label_len > 256 {
    return Err(std::io::Error::new(
      std::io::ErrorKind::InvalidData,
      "Wallet label exceeds 256 bytes",
    ));
  }
  let mut label = vec![0; label_len as usize];
  stream.read_exact(&mut label).await?;
  let payload_len = stream.read_u32_le().await?;
  if payload_len == 0 || payload_len > 64 * 1024 {
    return Err(std::io::Error::new(
      std::io::ErrorKind::InvalidData,
      "Invalid wallet payload length",
    ));
  }
  let mut payload = vec![0; payload_len as usize];
  stream.read_exact(&mut payload).await?;
  Ok((String::from_utf8_lossy(&label).into_owned(), payload, kind))
}

async fn handle(mut stream: TcpStream, bridge: Bridge) -> std::io::Result<()> {
  let mut prefix = [0; 4];
  tokio::time::timeout(
    std::time::Duration::from_secs(5),
    stream.read_exact(&mut prefix),
  )
  .await??;
  match &prefix {
    b"OPEN" => {
      open_wallet_popup(&bridge.app);
      return Ok(());
    }
    b"INFO" => {
      return stream.write_u32_le(http_port() as u32).await;
    }
    b"PKEY" => {
      let pubkey = bridge.wallet.0.lock().clone().unwrap_or_default();
      stream.write_u32_le(pubkey.len() as u32).await?;
      return stream.write_all(pubkey.as_bytes()).await;
    }
    _ => {}
  }
  let (label, payload, kind) = tokio::time::timeout(
    std::time::Duration::from_secs(5),
    read_frame(&mut stream, prefix),
  )
  .await??;
  let (response, receiver) = oneshot::channel();
  let id = uuid::Uuid::new_v4().to_string();
  let busy = {
    let mut pending = bridge.pending.lock();
    if pending.is_some() || super::lifecycle::stopped() {
      true
    } else {
      *pending = Some(PendingRequest {
        id: id.clone(),
        tx: payload,
        label,
        kind,
        response,
      });
      false
    }
  };
  if busy {
    return stream.write_u32_le(u32::MAX).await;
  }
  tracing::info!(request_id = %id, event = "SIGN_QUEUED", "Signing request accepted");
  bridge.notify.send_replace(());
  open_wallet_popup_for_signing(&bridge.app);

  let mut disconnected = [0; 1];
  let result = tokio::select! {
    result = tokio::time::timeout(std::time::Duration::from_secs(SIGN_TIMEOUT_SECS), receiver) => {
      match result {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("Signing request closed".into()),
        Err(_) => Err("Signing request timed out".into()),
      }
    }
    _ = stream.read(&mut disconnected) => Err("Game signing connection closed".into()),
  };
  match result {
    Ok(bytes) => {
      stream.write_u32_le(bytes.len() as u32).await?;
      stream.write_all(&bytes).await
    }
    Err(error) => {
      tracing::info!(request_id = %id, event = "SIGN_FAILED", %error, "Signing ended");
      super::requests::finish(&bridge.pending, &bridge.notify, &id, Err(error));
      stream.write_u32_le(u32::MAX).await
    }
  }
}
