use super::{PendingTx, PendingTxNotify};

pub(super) fn finish(
  pending: &PendingTx,
  notify: &PendingTxNotify,
  id: &str,
  result: Result<Vec<u8>, String>,
) -> bool {
  let mut guard = pending.lock();
  let request = match guard.take() {
    Some(request) if request.id == id => request,
    other => {
      *guard = other;
      return false;
    }
  };
  let _ = request.response.send(result);
  drop(guard);
  notify.send_replace(());
  true
}

/// The wallet address connected in the wallet popup, if any.
pub(crate) fn connected_wallet(app: &tauri::AppHandle) -> Option<String> {
  use tauri::Manager;
  app.state::<super::WalletPubkey>().0.lock().clone()
}

/// Queues a message-signing request for the connected wallet, opens the wallet popup for the
/// user to approve it, and returns the raw 64-byte signature. This is the same flow the game uses
/// over the TCP bridge; the admin panel calls it in-process.
pub(crate) async fn sign_message_in_popup(
  app: &tauri::AppHandle,
  label: &str,
  message: Vec<u8>,
) -> Result<Vec<u8>, String> {
  use tauri::Manager;
  let pending = app.state::<PendingTx>().inner().clone();
  let notify = app.state::<PendingTxNotify>().inner().clone();
  let (response, receiver) = tokio::sync::oneshot::channel();
  let id = uuid::Uuid::new_v4().to_string();
  {
    let mut guard = pending.lock();
    if guard.is_some() || super::lifecycle::stopped() {
      return Err("The wallet is busy with another signing request. Finish it and try again.".into());
    }
    *guard = Some(super::PendingRequest {
      id: id.clone(),
      tx: message,
      label: label.chars().take(120).collect(),
      kind: "message",
      response,
    });
  }
  notify.send_replace(());
  super::popup::open_wallet_popup_for_signing(app);
  match tokio::time::timeout(
    std::time::Duration::from_secs(super::SIGN_TIMEOUT_SECS),
    receiver,
  )
  .await
  {
    Ok(Ok(result)) => result,
    Ok(Err(_)) => Err("The signing request was closed".into()),
    Err(_) => {
      finish(&pending, &notify, &id, Err("Signing request timed out".into()));
      Err("The wallet did not answer in time".into())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::wallet::PendingRequest;
  use parking_lot::Mutex;
  use std::sync::Arc;

  fn request() -> (
    PendingTx,
    tokio::sync::oneshot::Receiver<Result<Vec<u8>, String>>,
    PendingTxNotify,
  ) {
    let (response, receiver) = tokio::sync::oneshot::channel();
    let (notify, _) = tokio::sync::watch::channel(());
    let pending = Arc::new(Mutex::new(Some(PendingRequest {
      id: "current".into(),
      tx: vec![1],
      label: "Test".into(),
      kind: "transaction",
      response,
    })));
    (pending, receiver, notify)
  }

  #[tokio::test]
  async fn late_resolution_cannot_consume_the_next_request(
  ) -> Result<(), Box<dyn std::error::Error>> {
    let (pending, mut receiver, notify) = request();
    assert!(!finish(&pending, &notify, "stale", Ok(vec![2])));
    assert!(matches!(
      receiver.try_recv(),
      Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
    assert!(finish(&pending, &notify, "current", Ok(vec![3])));
    assert_eq!(receiver.await??, vec![3]);
    assert!(pending.lock().is_none());
    assert!(!finish(&pending, &notify, "current", Ok(vec![4])));
    Ok(())
  }

  #[tokio::test]
  async fn cancellation_releases_the_signer_and_notifies_late_subscribers(
  ) -> Result<(), Box<dyn std::error::Error>> {
    let (pending, receiver, notify) = request();
    assert!(finish(&pending, &notify, "current", Err("Closed".into())));
    assert_eq!(
      receiver.await?.err().ok_or("expected cancellation")?,
      "Closed"
    );
    assert!(pending.lock().is_none());
    assert!(!notify.subscribe().has_changed()?);
    Ok(())
  }
}
