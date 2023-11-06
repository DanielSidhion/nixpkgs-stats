use std::{path::PathBuf, time::Duration};

use anyhow::{anyhow, Context};
use chrono::Utc;
use clap::Parser;
use data_state::DataState;
use graphql_client::{GraphQLQuery, Response};
use tokio::sync::oneshot;

use crate::data_state::CommitPack;

mod data_state;

pub type DateTime = chrono::DateTime<Utc>;
pub type GitObjectID = String;
pub type GitTimestamp = String;

#[derive(GraphQLQuery)]
#[graphql(
    // Downloaded directly from [GitHub's docs](https://docs.github.com/en/graphql/overview/public-schema).
    schema_path = "src/schema.docs.graphql",
    query_path = "src/query.graphql",
    response_derives = "Debug, Serialize"
)]
pub struct CommitStats;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory with the data fetched from GitHub.
    #[arg(short, long, default_value = "data")]
    data_dir: PathBuf,

    /// Running without backfill (the default) will only fetch any commit stats after the last time this command ran. When backfill is enabled, we'll also fetch any missing history.
    #[arg(short, long)]
    backfill: bool,
}

fn unpack_response(
    res: Response<commit_stats::ResponseData>,
) -> anyhow::Result<(
    commit_stats::CommitStatsRateLimit,
    bool,
    Option<String>,
    Vec<commit_stats::CommitStatsRepositoryRefTargetOnCommitHistoryNodes>,
)> {
    let data = res.data;

    if data.is_none() {
        println!("Errors: {:?}", res.errors);

        return Err(anyhow!("no data"));
    }

    let data = data.unwrap();

    let rate_limit = data.rate_limit.ok_or(anyhow!("no rate limit"))?;

    let top_level_ref = data
        .repository
        .and_then(|r| r.ref_)
        .and_then(|r| r.target)
        .map(|t| t.on);

    let (commits, has_next_page, next_page_cursor) =
        if let Some(commit_stats::CommitStatsRepositoryRefTargetOn::Commit(commit)) = top_level_ref
        {
            let commits = commit.history.nodes.unwrap_or_default();

            (
                commits.into_iter().flatten().collect(),
                commit.history.page_info.has_next_page,
                commit.history.page_info.end_cursor,
            )
        } else {
            println!("Top level is none!");
            (Vec::new(), false, None)
        };

    Ok((rate_limit, has_next_page, next_page_cursor, commits))
}

/// dotenvy throws an error if it can't find the `.env` file it's looking for. This function softens this behaviour by eating that particular error. When there's no `.env` file, it'll just be a no-op.
fn load_dotenvy_soft() -> anyhow::Result<()> {
    dotenvy::from_path(".env").or_else(|e| match e {
        dotenvy::Error::Io(io_error) if matches!(io_error.kind(), std::io::ErrorKind::NotFound) => {
            Ok(())
        }
        other => Err(other).context("Unable to read the .env file."),
    })
}

// TODO:
// - reset search whenever we've fetched too many records already (anything above 50k).
// - try to implement some additive increase/multiplicative decrease on number of records fetched.
// - verify that each commit has all the information it needs to have, and re-fetch if it doesnt.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_dotenvy_soft()?;
    let token = std::env::var("GITHUB_PERSONAL_ACCESS_TOKEN")
        .expect("the GITHUB_PERSONAL_ACCESS_TOKEN environment variable is not set");

    let args = Args::parse();

    let mut data_state = DataState::from_file(args.data_dir.join("state.json"))?;
    data_state.set_data_dir(args.data_dir);

    let client = reqwest::Client::builder()
        .user_agent("DanielSidhion/nixpkgs-stats-raw")
        .default_headers(
            std::iter::once((
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(&format!("bearer {}", token))?,
            ))
            .collect(),
        )
        .build()?;

    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    let shutdown_task = tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        println!("Detected ctrl+c signal, will signal a shutdown to the rest of the application.");
        shutdown_tx.send(()).unwrap();
    });

    let mut latest_cursor: Option<String> = None;

    loop {
        let query_variables = commit_stats::Variables {
            num_commits: 40,
            next_page_cursor: latest_cursor.clone(),
            since: None,
            // since: data_state
            //     .as_ref()
            //     .map(|data| data.latest_commit_time.to_owned()),
            until: data_state.earliest_commit_time(),
        };
        let request_body = CommitStats::build_query(query_variables);

        println!(
            "Making query...\n{}",
            serde_json::to_string_pretty(&request_body.variables)?
        );

        let res = client
            .post("https://api.github.com/graphql")
            .json(&request_body)
            .send()
            .await?;

        println!("Loading json...");

        let response_body: Response<commit_stats::ResponseData> = match res.json().await {
            Ok(v) => v,
            Err(err) if err.is_decode() || err.is_timeout() => {
                // TODO: if too many errors in sequence, save the pack and abort, then try running again with new variables.
                println!("Got decode error from GH API, will sleep 5s and try again...");
                println!("Sleeping for 5s...");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            Err(err) => return Err(err)?,
        };

        println!("Unpacking response and writing raw data...");

        let (rate_limit, has_next_page, next_page_cursor, commits) =
            match unpack_response(response_body) {
                Ok(v) => v,
                Err(err) if err.to_string() == "no data" => {
                    // TODO: if too many errors in sequence, save the pack and abort, then try running again with new variables.
                    println!("Got no data from GH API, will sleep 5s and try again...");
                    println!("Sleeping for 5s...");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
                Err(err) => {
                    return Err(err).context("Got an unexpected result from the GitHub API.")
                }
            };

        for commit in commits {
            data_state.add_commit(commit)?;
        }

        if !has_next_page
            || !matches!(
                shutdown_rx.try_recv(),
                Err(oneshot::error::TryRecvError::Empty),
            )
        {
            // In case we're here because the `try_recv()` call didn't tell us the channel was empty, it means we either got a ctrl+c signal or the channel dropped, in which case we're not listening for ctrl+c anymore, so let's just save and exit.
            data_state.save_curr_pack()?;
            break;
        } else {
            latest_cursor = next_page_cursor;
            println!("Next cursor is {:?}", latest_cursor);
            println!("Current pack date is {}", data_state.curr_pack_date());

            // TODO: sleep here if we're over the rate limit.
            println!("Rate limit: {:#?}", rate_limit);

            if rate_limit.remaining - rate_limit.cost < 0 {
                let sleep_duration = rate_limit.reset_at - Utc::now();
                let sleep_duration = sleep_duration.to_std()? + Duration::from_secs(30);
                println!(
                    "Sleeping for {}s for the rate limit to reset.",
                    sleep_duration.as_secs()
                );
                tokio::time::sleep(sleep_duration).await;
            } else {
                println!("Sleeping for 0.25s...");
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }
    }

    shutdown_task.abort();

    Ok(())
}
