use anyhow::Context as _;
use async_trait::async_trait;
use braid_http::{
    client::{
        ChannelCheckpoint, ChannelConfig, ChannelJournal, ChannelState, JournalBackend,
        ReliableChannel, RetryConfig,
    },
    traits::BraidNetwork,
    types::{BraidRequest, Update, Version},
    BraidError, BraidResponse, Result,
};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

#[derive(Default)]
struct Network {
    subscriptions: Mutex<Vec<BraidRequest>>,
    receivers: Mutex<VecDeque<async_channel::Receiver<Result<Update>>>>,
    puts: Mutex<Vec<BraidRequest>>,
    responses: Mutex<VecDeque<u16>>,
    pending: AtomicUsize,
    canceled: AtomicUsize,
}
struct InFlight<'a>(&'a Network);
impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        self.0.canceled.fetch_add(1, Ordering::SeqCst);
    }
}
#[async_trait]
impl BraidNetwork for Network {
    async fn fetch(&self, _: &str, request: BraidRequest) -> Result<BraidResponse> {
        lock(&self.puts).push(request);
        if self
            .pending
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
        {
            let _guard = InFlight(self);
            std::future::pending::<()>().await;
        }
        Ok(BraidResponse::new(
            lock(&self.responses).pop_front().unwrap_or(200),
            "",
        ))
    }
    async fn subscribe(
        &self,
        _: &str,
        request: BraidRequest,
    ) -> Result<async_channel::Receiver<Result<Update>>> {
        lock(&self.subscriptions).push(request);
        lock(&self.receivers)
            .pop_front()
            .ok_or(BraidError::SubscriptionClosed)
    }
}
fn config() -> ChannelConfig {
    ChannelConfig {
        put_timeout: Duration::from_millis(200),
        retry: RetryConfig::default()
            .with_initial_backoff(Duration::from_millis(1))
            .with_max_backoff(Duration::from_millis(2)),
        ..Default::default()
    }
}

#[tokio::test]
async fn sender_only_channel_commits_puts_without_opening_or_faking_a_subscription(
) -> anyhow::Result<()> {
    let network = Arc::new(Network::default());
    let sender = ReliableChannel::start_sender(
        network.clone(),
        "http://test/new-resource".into(),
        BraidRequest::new(),
        ChannelJournal::memory(),
        config(),
    )
    .context("unexpected failure")?;
    sender
        .queue(Update::snapshot(Version::new("created"), "content"), None)
        .await
        .context("unexpected failure")?;
    wait_for(|| !lock(&network.puts).is_empty() && sender.status().borrow().outstanding_puts == 0)
        .await;
    assert!(lock(&network.subscriptions).is_empty());
    assert_eq!(
        lock(&network.puts)[0].version,
        Some(vec![Version::new("created")])
    );
    sender.close().await;
    Ok(())
}

#[tokio::test]
async fn sender_receive_progress_and_application_are_one_fallible_transaction() -> anyhow::Result<()>
{
    let store = Arc::new(Mutex::new(FaultStore::default()));
    let journal = ChannelJournal::from_backend(FaultJournal(store.clone()))
        .await
        .context("unexpected failure")?;
    let network = Arc::new(Network::default());
    let sender = ReliableChannel::start_sender(
        network,
        "http://test/doc".into(),
        BraidRequest::new(),
        journal,
        config(),
    )
    .context("unexpected failure")?;
    wait_for(|| sender.status().borrow().state == ChannelState::Connecting).await;
    sender
        .checkpoint_received(
            vec![Version::new("accepted")],
            serde_json::json!({"through":"accepted"}),
        )
        .await
        .context("unexpected failure")?;
    let original = lock(&store).bytes.clone();
    lock(&store).fail_next = true;
    assert!(sender
        .checkpoint_received(
            vec![Version::new("not-persisted")],
            serde_json::json!({"through":"not-persisted"})
        )
        .await
        .is_err());
    assert_eq!(lock(&store).bytes, original);
    assert_eq!(
        sender.diagnostics().borrow().receive_frontier,
        vec![Version::new("accepted")]
    );
    sender.close().await;
    let journal = ChannelJournal::from_backend(FaultJournal(store))
        .await
        .context("unexpected failure")?;
    assert_eq!(
        journal.checkpoint().receive_frontier,
        vec![Version::new("accepted")]
    );
    assert_eq!(
        journal.checkpoint().application,
        Some(serde_json::json!({"through":"accepted"}))
    );
    Ok(())
}

#[tokio::test]
async fn sender_only_manual_ack_replays_the_same_identity_after_restart() -> anyhow::Result<()> {
    use braid_http::client::ChannelEvent;
    let store = Arc::new(Mutex::new(FaultStore::default()));
    let network = Arc::new(Network::default());
    let options = ChannelConfig {
        application_acknowledgments: true,
        ..config()
    };
    let journal = ChannelJournal::from_backend(FaultJournal(store.clone()))
        .await
        .context("unexpected failure")?;
    let sender = ReliableChannel::start_sender(
        network.clone(),
        "http://test/doc".into(),
        BraidRequest::new(),
        journal,
        options.clone(),
    )
    .context("unexpected failure")?;
    sender
        .queue(
            Update::snapshot(Version::new("uncertain"), "exact bytes"),
            None,
        )
        .await
        .context("unexpected failure")?;
    let event = sender
        .next_event()
        .await
        .context("unexpected failure")?
        .context("unexpected failure")?;
    assert!(
        matches!(event, ChannelEvent::PutAcknowledged { version, .. } if version == vec![Version::new("uncertain")])
    );
    assert_eq!(sender.status().borrow().outstanding_puts, 1);
    sender.close().await;
    let journal = ChannelJournal::from_backend(FaultJournal(store.clone()))
        .await
        .context("unexpected failure")?;
    let sender = ReliableChannel::start_sender(
        network.clone(),
        "http://test/doc".into(),
        BraidRequest::new(),
        journal,
        options,
    )
    .context("unexpected failure")?;
    let event = sender
        .next_event()
        .await
        .context("unexpected failure")?
        .context("unexpected failure")?;
    assert!(
        matches!(event, ChannelEvent::PutAcknowledged { version, .. } if version == vec![Version::new("uncertain")])
    );
    sender
        .commit_ack(
            vec![Version::new("uncertain")],
            Some(serde_json::json!({"ack":"uncertain"})),
        )
        .await
        .context("unexpected failure")?;
    sender.close().await;
    assert!(lock(&network.subscriptions).is_empty());
    let puts = lock(&network.puts);
    assert_eq!(puts.len(), 2);
    assert_eq!(
        braid_http::protocol::encode_request(&puts[0]).context("unexpected failure")?,
        braid_http::protocol::encode_request(&puts[1]).context("unexpected failure")?
    );
    Ok(())
}

#[derive(Default)]
struct FaultStore {
    bytes: Option<Vec<u8>>,
    fail_next: bool,
    fail_dequeue: bool,
}
struct FaultJournal(Arc<Mutex<FaultStore>>);
#[async_trait]
impl JournalBackend for FaultJournal {
    async fn load(&mut self) -> Result<Option<Vec<u8>>> {
        Ok(lock(&self.0).bytes.clone())
    }
    async fn store(&mut self, bytes: &[u8]) -> Result<()> {
        let mut store = lock(&self.0);
        let next: ChannelCheckpoint = serde_json::from_slice(bytes)?;
        let previous: ChannelCheckpoint = store
            .bytes
            .as_deref()
            .map(serde_json::from_slice)
            .transpose()?
            .unwrap_or_default();
        let dequeue = store.fail_dequeue && !previous.outbox.is_empty() && next.outbox.is_empty();
        if std::mem::take(&mut store.fail_next) || dequeue {
            store.fail_dequeue = false;
            return Err(std::io::Error::from(std::io::ErrorKind::StorageFull).into());
        }
        store.bytes = Some(bytes.to_vec());
        Ok(())
    }
}

#[test]
fn missing_runtime_is_a_capability_error_before_network_work() -> anyhow::Result<()> {
    fn retains_native_auto_traits<T: Send + Sync>() {}
    retains_native_auto_traits::<ChannelJournal>();
    retains_native_auto_traits::<ReliableChannel>();
    let network = Arc::new(Network::default());
    let result = ReliableChannel::start(
        network.clone(),
        "http://test/doc".into(),
        BraidRequest::new(),
        ChannelJournal::memory(),
        config(),
    );
    assert!(matches!(
        result,
        Err(BraidError::CapabilityUnavailable {
            capability: "Tokio runtime"
        })
    ));
    assert!(lock(&network.subscriptions).is_empty());
    assert!(lock(&network.puts).is_empty());
    Ok(())
}

#[tokio::test]
async fn failed_backend_commit_does_not_ack_queue_or_cursor_and_recovery_is_atomic(
) -> anyhow::Result<()> {
    let store = Arc::new(Mutex::new(FaultStore::default()));
    let mut journal = ChannelJournal::from_backend(FaultJournal(store.clone()))
        .await
        .context("unexpected failure")?;
    let original = serde_json::json!({"text":"before"});
    journal
        .initialize_application(vec![], original.clone())
        .await
        .context("unexpected failure")?;
    let network = Arc::new(Network::default());
    let (sender, receiver) = async_channel::bounded(1);
    lock(&network.receivers).push_back(receiver);
    network.pending.store(1, Ordering::SeqCst);
    let channel = start(network.clone(), journal, config());
    wait_for(|| channel.status().borrow().state == ChannelState::Online).await;
    let baseline = lock(&store).bytes.clone();
    lock(&store).fail_next = true;
    let edit = Update::snapshot(Version::new("immutable-edit"), "after");
    let application = serde_json::json!({"text":"after","pending_identity":"immutable-edit"});
    assert!(channel
        .queue(edit.clone(), Some(application.clone()))
        .await
        .is_err());
    assert_eq!(lock(&store).bytes, baseline);
    assert!(lock(&network.puts).is_empty());
    assert_eq!(channel.status().borrow().outstanding_puts, 0);

    channel
        .queue(edit.clone(), Some(application.clone()))
        .await
        .context("unexpected failure")?;
    wait_for(|| !lock(&network.puts).is_empty()).await;
    let queued = lock(&store).bytes.clone();
    sender
        .send(Ok(Update::snapshot(Version::new("received"), "remote")))
        .await
        .context("unexpected failure")?;
    channel
        .next()
        .await
        .context("unexpected failure")?
        .context("unexpected failure")?;
    lock(&store).fail_next = true;
    assert!(channel
        .applied(
            vec![Version::new("received")],
            Some(serde_json::json!({"text":"remote"}))
        )
        .await
        .is_err());
    wait_for(|| channel.status().borrow().state == ChannelState::RepairRequired).await;
    channel.close().await;
    assert_eq!(lock(&store).bytes, queued);
    let reopened = ChannelJournal::from_backend(FaultJournal(store))
        .await
        .context("unexpected failure")?;
    assert!(reopened.checkpoint().receive_frontier.is_empty());
    assert_eq!(reopened.checkpoint().application, Some(application));
    assert_eq!(
        serde_json::to_value(&reopened.checkpoint().outbox[0]).context("unexpected failure")?,
        serde_json::to_value(edit).context("unexpected failure")?
    );
    assert_eq!(network.canceled.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn failed_ack_persistence_replays_the_original_put_after_restart() -> anyhow::Result<()> {
    let store = Arc::new(Mutex::new(FaultStore {
        fail_dequeue: true,
        ..Default::default()
    }));
    let journal = ChannelJournal::from_backend(FaultJournal(store.clone()))
        .await
        .context("unexpected failure")?;
    let network = Arc::new(Network::default());
    let (_sender, receiver) = async_channel::bounded(1);
    lock(&network.receivers).push_back(receiver);
    let channel = start(network.clone(), journal, config());
    let edit = Update::snapshot(Version::new("uncertain-ack"), "content");
    channel
        .queue(edit, Some(serde_json::json!({"text":"content"})))
        .await
        .context("unexpected failure")?;
    wait_for(|| channel.status().borrow().state == ChannelState::RepairRequired).await;
    channel.close().await;
    let journal = ChannelJournal::from_backend(FaultJournal(store.clone()))
        .await
        .context("unexpected failure")?;
    assert_eq!(journal.checkpoint().outbox.len(), 1);
    let (_sender, receiver) = async_channel::bounded(1);
    lock(&network.receivers).push_back(receiver);
    let channel = start(network.clone(), journal, config());
    wait_for(|| lock(&network.puts).len() == 2 && channel.status().borrow().outstanding_puts == 0)
        .await;
    channel.close().await;
    {
        let puts = lock(&network.puts);
        assert_eq!(puts[0].method, puts[1].method);
        assert_eq!(
            braid_http::protocol::encode_request(&puts[0]).context("unexpected failure")?,
            braid_http::protocol::encode_request(&puts[1]).context("unexpected failure")?
        );
    }
    let recovered = ChannelJournal::from_backend(FaultJournal(store))
        .await
        .context("unexpected failure")?;
    assert!(recovered.checkpoint().outbox.is_empty());
    assert_eq!(
        recovered.checkpoint().application,
        Some(serde_json::json!({"text":"content"}))
    );
    Ok(())
}

#[tokio::test]
async fn scoped_channel_credentials_reach_transport_without_entering_recovery_bytes(
) -> anyhow::Result<()> {
    use braid_http::client::{OriginCredentials, ScopedCredentials};
    let path = std::env::temp_dir().join(format!(
        "braid-scoped-channel-{}.json",
        uuid::Uuid::new_v4()
    ));
    let network = Arc::new(Network::default());
    let (_sender, receiver) = async_channel::bounded(1);
    lock(&network.receivers).push_back(receiver);
    let credentials = ScopedCredentials::new(
        "http://test",
        OriginCredentials {
            bearer_token: Some("private-channel-token".into()),
            cookie: None,
        },
    )
    .context("unexpected failure")?;
    let request = credentials
        .authenticate("http://test/doc", BraidRequest::new())
        .context("unexpected failure")?;
    let channel = ReliableChannel::start(
        network.clone(),
        "http://test/doc".into(),
        request,
        ChannelJournal::open(&path)
            .await
            .context("unexpected failure")?,
        config(),
    )
    .context("unexpected failure")?;
    channel
        .queue(Update::snapshot(Version::new("one"), "content"), None)
        .await
        .context("unexpected failure")?;
    wait_for(|| !lock(&network.puts).is_empty()).await;
    assert_eq!(
        lock(&network.puts)[0].extra_headers["authorization"],
        "Bearer private-channel-token"
    );
    channel.close().await;
    let bytes = std::fs::read(&path).context("unexpected failure")?;
    assert!(!String::from_utf8_lossy(&bytes).contains("private-channel-token"));
    std::fs::remove_file(&path).context("unexpected failure")?;
    std::fs::remove_file(path.with_extension("lock")).context("unexpected failure")?;
    Ok(())
}

#[tokio::test]
async fn transport_headers_cannot_enter_an_outbox_and_old_bytes_are_retained_for_migration(
) -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!(
        "braid-secret-boundary-{}.json",
        uuid::Uuid::new_v4()
    ));
    let network = Arc::new(Network::default());
    let (_sender, receiver) = async_channel::bounded(1);
    lock(&network.receivers).push_back(receiver);
    let journal = ChannelJournal::open(&path)
        .await
        .context("unexpected failure")?;
    let channel = start(network.clone(), journal, config());
    let mut update = Update::snapshot(Version::new("uncertain"), "content");
    update
        .extra_headers
        .insert("authorization".into(), "Bearer private-value".into());
    assert!(channel.queue(update.clone(), None).await.is_err());
    channel.close().await;
    assert!(lock(&network.puts).is_empty());
    let mut checkpoint = braid_http::client::ChannelCheckpoint::default();
    checkpoint.outbox.push_back(update);
    let original = serde_json::to_vec(&checkpoint).context("unexpected failure")?;
    std::fs::write(&path, &original).context("unexpected failure")?;
    assert!(matches!(
        ChannelJournal::open(&path).await,
        Err(BraidError::Config(_))
    ));
    assert_eq!(
        std::fs::read(&path).context("unexpected failure")?,
        original,
        "never silently rewrite an uncertain PUT during recovery"
    );
    std::fs::remove_file(&path).context("unexpected failure")?;
    std::fs::remove_file(path.with_extension("lock")).context("unexpected failure")?;
    Ok(())
}
fn start(network: Arc<Network>, journal: ChannelJournal, config: ChannelConfig) -> ReliableChannel {
    ReliableChannel::start(
        network,
        "http://test/doc".into(),
        BraidRequest::new(),
        journal,
        config,
    )
    .unwrap_or_else(|error| panic!("channel failed to start: {error}"))
}
async fn wait_for(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("condition was not reached in time"));
}

#[tokio::test]
async fn subscription_failure_cancels_put_and_replays_exact_payload() -> anyhow::Result<()> {
    let network = Arc::new(Network::default());
    let (tx1, rx1) = async_channel::bounded(2);
    let (_tx2, rx2) = async_channel::bounded(2);
    lock(&network.receivers).extend([rx1, rx2]);
    network.pending.store(1, Ordering::SeqCst);
    let mut journal = ChannelJournal::memory();
    journal
        .initialize_application(vec![Version::new("known")], serde_json::json!({}))
        .await
        .context("unexpected failure")?;
    let channel = start(
        network.clone(),
        journal,
        ChannelConfig {
            resume_local_frontier: true,
            ..config()
        },
    );
    let update = Update::snapshot(Version::new("stable-id"), "\u{1f600} immutable")
        .with_parents(vec![Version::new("known")]);
    channel
        .queue(update, None)
        .await
        .context("unexpected failure")?;
    wait_for(|| lock(&network.puts).len() == 1).await;
    tx1.send(Err(BraidError::Http("connection reset".into())))
        .await
        .context("unexpected failure")?;
    wait_for(|| lock(&network.puts).len() == 2).await;
    assert_eq!(network.canceled.load(Ordering::SeqCst), 1);
    let puts = lock(&network.puts).clone();
    assert_eq!(puts[0].version, puts[1].version);
    assert_eq!(puts[0].body, puts[1].body);
    assert_eq!(
        lock(&network.subscriptions)[1].parents,
        Some(vec![Version::new("known")]),
        "Reconnect cannot name a version which may never have reached the server"
    );
    wait_for(|| channel.status().borrow().outstanding_puts == 0).await;
    channel.close().await;
    Ok(())
}

#[tokio::test]
async fn lost_ack_restart_retains_identity_application_and_writer_exclusion() -> anyhow::Result<()>
{
    let path = std::env::temp_dir().join(format!("braid-channel-{}.json", uuid::Uuid::new_v4()));
    let journal = ChannelJournal::open(&path)
        .await
        .context("unexpected failure")?;
    assert!(ChannelJournal::open(&path).await.is_err());
    let network = Arc::new(Network::default());
    let (_tx, rx) = async_channel::bounded(2);
    lock(&network.receivers).push_back(rx);
    network.pending.store(1, Ordering::SeqCst);
    let channel = start(network.clone(), journal, config());
    channel
        .queue(
            Update::snapshot(Version::new("survives-crash"), "local"),
            Some(serde_json::json!({"text":"local"})),
        )
        .await
        .context("unexpected failure")?;
    wait_for(|| lock(&network.puts).len() == 1).await;
    channel.close().await;
    let journal = ChannelJournal::open(&path)
        .await
        .context("unexpected failure")?;
    assert_eq!(
        journal.checkpoint().outbox[0].version,
        vec![Version::new("survives-crash")]
    );
    assert_eq!(
        journal
            .checkpoint()
            .application
            .as_ref()
            .context("unexpected failure")?["text"],
        "local"
    );
    let (_tx2, rx2) = async_channel::bounded(2);
    lock(&network.receivers).push_back(rx2);
    let channel = start(network.clone(), journal, config());
    wait_for(|| channel.status().borrow().outstanding_puts == 0).await;
    channel.close().await;
    let journal = ChannelJournal::open(&path)
        .await
        .context("unexpected failure")?;
    assert!(journal.checkpoint().outbox.is_empty());
    drop(journal);
    std::fs::remove_file(&path).context("unexpected failure")?;
    std::fs::remove_file(path.with_extension("lock")).context("unexpected failure")?;
    Ok(())
}

#[tokio::test]
async fn cursor_advances_only_after_application_commit_and_bad_syntax_is_terminal(
) -> anyhow::Result<()> {
    let network = Arc::new(Network::default());
    let (tx, rx) = async_channel::bounded(2);
    lock(&network.receivers).push_back(rx);
    let channel = start(network.clone(), ChannelJournal::memory(), config());
    tx.send(Ok(Update::snapshot(Version::new("received"), "text")))
        .await
        .context("unexpected failure")?;
    let update = channel
        .next()
        .await
        .context("unexpected failure")?
        .context("unexpected failure")?;
    assert!(channel
        .status()
        .borrow()
        .durable_receive_frontier
        .is_empty());
    assert!(matches!(
        channel.applied(vec![Version::new("wrong")], None).await,
        Err(BraidError::Protocol(_))
    ));
    channel
        .applied(update.version.clone(), None)
        .await
        .context("unexpected failure")?;
    assert_eq!(
        channel.status().borrow().durable_receive_frontier,
        update.version
    );
    tx.send(Err(BraidError::HeaderParse(
        "invalid structured field".into(),
    )))
    .await
    .context("unexpected failure")?;
    assert!(matches!(
        channel.next().await,
        Some(Err(BraidError::HeaderParse(_)))
    ));
    assert_eq!(
        channel.status().borrow().state,
        ChannelState::RepairRequired
    );
    assert_eq!(lock(&network.subscriptions).len(), 1);
    channel.close().await;
    Ok(())
}

#[tokio::test]
async fn bounded_queue_rejects_reuse_and_overflow_without_dropping_original() -> anyhow::Result<()>
{
    let network = Arc::new(Network::default());
    let (_tx, rx) = async_channel::bounded(2);
    lock(&network.receivers).push_back(rx);
    network.pending.store(1, Ordering::SeqCst);
    let channel = start(
        network,
        ChannelJournal::memory(),
        ChannelConfig {
            max_pending: 1,
            put_timeout: Duration::from_secs(60),
            ..config()
        },
    );
    channel
        .queue(Update::snapshot(Version::new("one"), "original"), None)
        .await
        .context("unexpected failure")?;
    assert!(matches!(
        channel
            .queue(Update::snapshot(Version::new("one"), "changed"), None)
            .await,
        Err(BraidError::Protocol(_))
    ));
    assert!(matches!(
        channel
            .queue(Update::snapshot(Version::new("two"), "second"), None)
            .await,
        Err(BraidError::ResourceLimit { .. })
    ));
    assert_eq!(channel.status().borrow().outstanding_puts, 1);
    channel.close().await;
    Ok(())
}

#[test]
fn explicit_negotiated_representation_type_survives_patch_encoding() -> anyhow::Result<()> {
    let request = BraidRequest::new()
        .with_method("PUT")
        .with_version(Version::new("v1"))
        .with_content_type("text/plain")
        .with_header("Repr-Type", "text/html")
        .with_patches(vec![braid_http::types::Patch::text("[0:0]", "text")]);
    let (headers, _) =
        braid_http::protocol::encode_request(&request).context("unexpected failure")?;
    assert_eq!(headers["repr-type"], "text/html");
    assert_eq!(headers["content-type"], "application/http-patches; count=1");
    Ok(())
}

#[tokio::test]
#[ignore = "Only invoked by the process-crash regression"]
async fn child_journal_writer() -> anyhow::Result<()> {
    let Some(path) = std::env::var_os("BRAID_TEST_JOURNAL") else {
        return Ok(());
    };
    let path = std::path::PathBuf::from(path);
    let journal = ChannelJournal::open(&path)
        .await
        .context("unexpected failure")?;
    let network = Arc::new(Network::default());
    let (_tx, rx) = async_channel::bounded(1);
    lock(&network.receivers).push_back(rx);
    network.pending.store(1, Ordering::SeqCst);
    let channel = start(
        network.clone(),
        journal,
        ChannelConfig {
            put_timeout: Duration::from_secs(60),
            ..config()
        },
    );
    channel
        .queue(
            Update::snapshot(
                Version::new("process-stable"),
                "survives process termination",
            ),
            Some(serde_json::json!({"text":"survives process termination"})),
        )
        .await
        .context("unexpected failure")?;
    wait_for(|| lock(&network.puts).len() == 1).await;
    std::fs::write(path.with_extension("ready"), b"ready").context("unexpected failure")?;
    std::future::pending::<()>().await;
    Ok(())
}

#[tokio::test]
async fn killed_process_releases_lease_and_replays_its_durable_unacknowledged_edit(
) -> anyhow::Result<()> {
    let path = std::env::temp_dir().join(format!("braid-process-{}.json", uuid::Uuid::new_v4()));
    let mut child =
        tokio::process::Command::new(std::env::current_exe().context("unexpected failure")?)
            .args(["--ignored", "--exact", "child_journal_writer"])
            .env("BRAID_TEST_JOURNAL", &path)
            .kill_on_drop(true)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("unexpected failure")?;
    wait_for(|| path.with_extension("ready").exists()).await;
    assert!(ChannelJournal::open(&path).await.is_err());
    child.kill().await.context("unexpected failure")?;
    child.wait().await.context("unexpected failure")?;
    let journal = ChannelJournal::open(&path)
        .await
        .context("unexpected failure")?;
    assert_eq!(
        journal.checkpoint().outbox[0].version,
        vec![Version::new("process-stable")]
    );
    assert_eq!(
        journal.checkpoint().outbox[0].body_str(),
        Some("survives process termination")
    );
    let network = Arc::new(Network::default());
    let (_tx, rx) = async_channel::bounded(1);
    lock(&network.receivers).push_back(rx);
    let channel = start(network.clone(), journal, config());
    wait_for(|| channel.status().borrow().outstanding_puts == 0).await;
    assert_eq!(
        lock(&network.puts)[0].version,
        Some(vec![Version::new("process-stable")])
    );
    channel.close().await;
    for file in [
        &path,
        &path.with_extension("lock"),
        &path.with_extension("ready"),
    ] {
        std::fs::remove_file(file).context("unexpected failure")?;
    }
    Ok(())
}

/// A poisoned test lock still guards consistent data; keep going so the real failure shows.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
