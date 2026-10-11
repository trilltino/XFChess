use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use tokio::{sync::watch, task::JoinSet};

use super::super::scenario::{Scenario, Stage};
use super::{
    http::RequestContext,
    streams::{merge_streams, start_streams},
    template::render,
    GameResult, Variables,
};

pub(super) async fn game(
    client: &reqwest::Client,
    scenario: &Scenario,
    stage: &Stage,
    index: u32,
    run_id: &str,
    start: Instant,
) -> GameResult {
    let mut output = GameResult::default();
    let mut variables = scenario.variables.clone();
    if !scenario.actors.is_empty() {
        variables.extend(scenario.actors[index as usize % scenario.actors.len()].clone());
    }
    // Resolve actor environment references once; captures then remain private to this game.
    let resolved = variables
        .iter()
        .map(|(k, v)| Ok((k.clone(), render(v, &variables)?)))
        .collect::<Result<Variables>>();
    let Ok(mut variables) = resolved else {
        return output;
    };
    variables.extend(BTreeMap::from([
        ("run_id".into(), Value::String(run_id.into())),
        ("stage".into(), Value::String(stage.name.clone())),
        ("game_index".into(), Value::from(index)),
        (
            "game_id".into(),
            Value::String(format!("bench-{run_id}-{}-{index}", stage.name)),
        ),
    ]));
    let (stop, receiver) = watch::channel(false);
    let mut subscriptions = JoinSet::new();
    let context = RequestContext {
        client,
        scenario,
        game_index: index,
        start,
    };
    let flow = async {
        for step in &scenario.workflow.setup {
            context.send(step, &mut variables, 0.0, &mut output).await?;
        }
        start_streams(
            client,
            scenario,
            &variables,
            &scenario.workflow.spectators,
            &receiver,
            &mut subscriptions,
        )
        .await?;
        let move_start = Instant::now();
        for (ply, value) in scenario.workflow.moves.iter().enumerate() {
            let offset = scenario
                .workflow
                .move_interval_ms
                .checked_mul(ply as u64)
                .context("move pacing overflow")?;
            let deadline = move_start
                .checked_add(Duration::from_millis(offset))
                .context("move deadline overflow")?;
            tokio::time::sleep_until(deadline.into()).await;
            let lag = deadline.elapsed().as_secs_f64() * 1000.0;
            variables.insert("move".into(), render(value, &variables)?);
            variables.insert("move_index".into(), Value::from(ply));
            for step in &scenario.workflow.each_move {
                context.send(step, &mut variables, lag, &mut output).await?;
            }
        }
        for step in &scenario.workflow.finish {
            context.send(step, &mut variables, 0.0, &mut output).await?;
        }
        Ok::<_, anyhow::Error>(())
    }
    .await;
    output.completed = flow.is_ok();
    let _ = stop.send(true);
    while let Some(value) = subscriptions.join_next().await {
        match value {
            Ok(value) => merge_streams(&mut output.streams, value),
            Err(_) => output.streams.errors += 1,
        }
    }
    for step in &scenario.workflow.cleanup {
        if context
            .send(step, &mut variables, 0.0, &mut output)
            .await
            .is_err()
        {
            output.cleanup_failed = true;
        }
    }
    output
}
