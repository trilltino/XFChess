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
