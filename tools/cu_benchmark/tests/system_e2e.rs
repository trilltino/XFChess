use anyhow::Context;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use cu_benchmark::system::{
    report::{self, Capacity},
    runner,
    scenario::Scenario,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

#[derive(Default)]
struct Counters {
    calls: AtomicU32,
    cleanup: AtomicU32,
    retried: AtomicU32,
}

async fn create(State(state): State<Arc<Counters>>, Json(body): Json<Value>) -> Json<Value> {
    state.calls.fetch_add(1, Ordering::Relaxed);
    Json(json!({"id":body["index"],"ok":true,"secret":"never-export-this-token"}))
}

async fn move_request(
    State(state): State<Arc<Counters>>,
    Path(id): Path<u32>,
) -> (axum::http::StatusCode, Json<Value>) {
    state.calls.fetch_add(1, Ordering::Relaxed);
    if id == 0 && state.retried.fetch_add(1, Ordering::Relaxed) == 0 {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"ok":false})),
        );
    }
    (axum::http::StatusCode::OK, Json(json!({"ok":id != 5})))
}

async fn cleanup(State(state): State<Arc<Counters>>) -> Json<Value> {
    state.calls.fetch_add(1, Ordering::Relaxed);
    state.cleanup.fetch_add(1, Ordering::Relaxed);
    Json(json!({"ok":true}))
}

async fn metrics(State(state): State<Arc<Counters>>) -> String {
    format!(
        "# TYPE calls_total counter\ncalls_total {}\nmemory_bytes 1234\n",
        state.calls.load(Ordering::Relaxed)
    )
}

async fn fixture() -> anyhow::Result<(
    Scenario,
    Arc<Counters>,
    tokio::task::JoinHandle<std::io::Result<()>>,
)> {
    let counters = Arc::new(Counters::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context(concat!(file!(), ":", line!()))?;
    let base = format!(
        "http://{}",
        listener
            .local_addr()
            .context(concat!(file!(), ":", line!()))?
    );
    let app = Router::new()
        .route("/create", post(create))
        .route("/move/{id}", post(move_request))
        .route("/cleanup", post(cleanup))
        .route("/metrics", get(metrics))
        .with_state(counters.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await });
    let scenario = serde_json::from_value(json!({
        "name":"fixture","backend_url":base,"stages":[{"name":"four","games":12,"concurrency":4}],
        "sampling_ms":20,"baseline_ms":25,"timeout_ms":1000,
        "workflow":{
            "setup":[{"name":"create","method":"POST","path":"/create","body":{"index":"${game_index}"},
                "capture":{"id":"/id","token":"/secret"},"expect":{"/ok":true},"capital_delta_lamports":1000}],
            "moves":[{"uci":"f2f3"},{"uci":"e7e5"}],"move_interval_ms":10,
            "each_move":[{"name":"move","method":"POST","path":"/move/${id}",
                "headers":{"Authorization":"Bearer ${token}"},"expect":{"/ok":true},"idempotent":true,"retries":1}],
            "cleanup":[{"name":"cleanup","method":"POST","path":"/cleanup","expect":{"/ok":true},"capital_delta_lamports":-1000}]
        },
        "telemetry":[{"name":"calls","url":format!("{base}/metrics"),"metric":"calls_total","kind":"counter","unit":"requests","required":true},
            {"name":"memory","url":format!("{base}/metrics"),"metric":"memory_bytes","kind":"gauge","unit":"bytes","required":true,"max":2000}],
        "limits":{"request_p95_ms":1000,"error_ratio":0.1,"min_duration_ms":0,"max_schedule_lag_ms":1000},
        "pricing":{"configured":true,"currency":"GBP","monthly_games":120,"fixed":[{"name":"VPS","monthly_minor_units":2000}],
            "metered":[{"name":"API","metric":"calls","quantity_per_unit":100,"minor_units_per_unit":10,"included_monthly_quantity":100}]}
    })).context(concat!(file!(), ":", line!()))?;
    Ok((scenario, counters, task))
}

#[tokio::test]
async fn captures_retries_domain_failures_cleanup_telemetry_and_pricing() -> anyhow::Result<()> {
    let (scenario, counters, task) = fixture().await?;
    scenario
        .validate()
        .context(concat!(file!(), ":", line!()))?;
    let observed =
        runner::execute(&scenario, false)
            .await
            .context(concat!(file!(), ":", line!()))?;
    assert_eq!(observed[0].completed_games, 11);
    assert_eq!(counters.cleanup.load(Ordering::Relaxed), 12);
    assert_eq!(observed[0].peak_concurrency, 4);
    assert!(observed[0].attempts.iter().any(|a| a.attempt == 1));
    assert!(observed[0]
        .attempts
        .iter()
        .any(|a| a.error.as_deref() == Some("expectation")));
    let report = report::build(&scenario, observed).context(concat!(file!(), ":", line!()))?;
    let stage = &report.stages[0];
    assert_eq!(stage.capacity, Capacity::Fail);
    assert_eq!(
        stage.metrics["calls"].delta,
        Some(f64::from(counters.calls.load(Ordering::Relaxed)))
    );
    assert_eq!(stage.operating_costs.fixed_monthly_minor_units, Some(2000));
    assert!(stage.operating_costs.metered[0]
        .projected_monthly_quantity
        .is_some());
    assert!(stage.estimated_monthly_total_minor_units.is_none());
    assert!(stage.modeled_peak_reserved_capital_lamports.is_some());
    assert!(!serde_json::to_string(&report)
        .context(concat!(file!(), ":", line!()))?
        .contains("never-export-this-token"));
    task.abort();
    Ok(())
}

#[tokio::test]
async fn missing_required_resource_data_prevents_a_capacity_pass() -> anyhow::Result<()> {
    let (mut scenario, _counters, task) = fixture().await?;
    scenario.stages[0].games = 4;
    scenario.telemetry[1].metric = "missing_metric".into();
    let observed =
        runner::execute(&scenario, false)
            .await
            .context(concat!(file!(), ":", line!()))?;
    let report = report::build(&scenario, observed).context(concat!(file!(), ":", line!()))?;
    assert_eq!(report.stages[0].capacity, Capacity::Inconclusive);
    assert!(!report.stages[0].metrics["memory"].complete);
    assert_eq!(report.max_passing_observed_concurrency, None);
    task.abort();
    Ok(())
}

#[tokio::test]
async fn transaction_scenarios_require_explicit_execution_opt_in() -> anyhow::Result<()> {
    let (mut scenario, counters, task) = fixture().await?;
    scenario.sends_transactions = true;
    assert!(runner::execute(&scenario, false).await.is_err());
    assert_eq!(counters.calls.load(Ordering::Relaxed), 0);
    task.abort();
    Ok(())
}

#[tokio::test]
async fn missing_move_environment_is_rejected_before_setup() -> anyhow::Result<()> {
    let (mut scenario, counters, task) = fixture().await?;
    scenario.workflow.moves[0] = json!({"token":"${env:CU_BENCH_MISSING_MOVE_TOKEN_E2E}"});
    assert!(runner::execute(&scenario, false).await.is_err());
    assert_eq!(counters.calls.load(Ordering::Relaxed), 0);
    task.abort();
    Ok(())
}

#[test]
fn examples_validate_offline_and_repricing_does_not_change_workload_identity() -> anyhow::Result<()>
{
    let provider_example: Scenario = serde_json::from_str(include_str!(
        "../scenarios/hetzner-triton.json"
    ))
    .context(concat!(file!(), ":", line!()))?;
    provider_example
        .validate()
        .context(concat!(file!(), ":", line!()))?;
    let mut scenario: Scenario = serde_json::from_str(include_str!("../scenarios/backend.json"))
        .context(concat!(file!(), ":", line!()))?;
    scenario
        .validate()
        .context(concat!(file!(), ":", line!()))?;
    let before = report::fingerprint(&scenario).context(concat!(file!(), ":", line!()))?;
    scenario.pricing.monthly_games = 500;
    scenario.limits.request_p95_ms = 200.0;
    assert_eq!(
        before,
        report::fingerprint(&scenario).context(concat!(file!(), ":", line!()))?
    );
    scenario.workflow.move_interval_ms += 1;
    assert_ne!(
        before,
        report::fingerprint(&scenario).context(concat!(file!(), ":", line!()))?
    );
    scenario.workflow.each_move[0].idempotent = false;
    assert!(scenario.validate().is_err());
    Ok(())
}

#[tokio::test]
async fn cli_reprices_saved_observations_without_contacting_the_backend() -> anyhow::Result<()> {
    let (mut scenario, _counters, task) = fixture().await?;
    let observations =
        runner::execute(&scenario, false)
            .await
            .context(concat!(file!(), ":", line!()))?;
    let measured =
        report::build(&scenario, observations).context(concat!(file!(), ":", line!()))?;
    task.abort();
    let directory = tempfile::tempdir().context(concat!(file!(), ":", line!()))?;
    let config = directory.path().join("scenario.json");
    let source = directory.path().join("measured.json");
    let output = directory.path().join("repriced.json");
    std::fs::write(
        &source,
        serde_json::to_vec(&measured).context(concat!(file!(), ":", line!()))?,
    )
    .context(concat!(file!(), ":", line!()))?;
    scenario.pricing.fixed[0].monthly_minor_units = 6000;
    std::fs::write(
        &config,
        serde_json::to_vec(&scenario).context(concat!(file!(), ":", line!()))?,
    )
    .context(concat!(file!(), ":", line!()))?;
    let result = tokio::process::Command::new(env!("CARGO_BIN_EXE_cu_benchmark"))
        .arg("--scenario")
        .arg(&config)
        .arg("--observations")
        .arg(&source)
        .arg("--export")
        .arg(&output)
        .output()
        .await
        .context(concat!(file!(), ":", line!()))?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let repriced: report::SystemReport =
        serde_json::from_slice(&std::fs::read(output).context(concat!(file!(), ":", line!()))?)
            .context(concat!(file!(), ":", line!()))?;
    assert_eq!(
        repriced.stages[0].operating_costs.fixed_monthly_minor_units,
        Some(6000)
    );
    assert_eq!(
        repriced.stages[0].observed.completed_games,
        measured.stages[0].observed.completed_games
    );
    assert_eq!(repriced.workload_fingerprint, measured.workload_fingerprint);
    scenario.workflow.move_interval_ms += 1;
    std::fs::write(
        &config,
        serde_json::to_vec(&scenario).context(concat!(file!(), ":", line!()))?,
    )
    .context(concat!(file!(), ":", line!()))?;
    let result = tokio::process::Command::new(env!("CARGO_BIN_EXE_cu_benchmark"))
        .arg("--scenario")
        .arg(config)
        .arg("--observations")
        .arg(source)
        .output()
        .await
        .context(concat!(file!(), ":", line!()))?;
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("workload differs"));
    Ok(())
}
