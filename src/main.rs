mod api;

use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

use anyhow::Result;
use axum::{middleware, routing::get};
use axum_auth_api_key::auth_middleware;
use clap::{Parser, Subcommand};
use ferrochain_anthropic_completion::{AnthropicCompletion, Model};
use ferrochain_voyageai_embedder::{EmbeddingInputType, EmbeddingModel, VoyageAiEmbedder};
use synx::{executor::Executor, Synx};
use synx_heed_database::{heed::EnvOpenOptions, SynxHeedDatabase};
use synx_in_memory_database::SynxInMemory;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct TokioExecutor;

impl Executor for TokioExecutor {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) {
        tokio::spawn(future);
    }
}

#[derive(Parser)]
struct Cli {
    #[clap(long, default_value = "0.0.0.0")]
    host: String,
    #[clap(long, default_value = "3000")]
    port: u16,
    #[clap(long, env = "SYNX_API_KEY")]
    api_key: String,
    #[clap(subcommand)]
    database: Database,
}

#[derive(Default, Subcommand)]
enum Database {
    Heed {
        #[clap(long)]
        path: PathBuf,
        #[clap(long, default_value = "false")]
        regenerate: bool,
    },
    #[default]
    InMemory,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!(
                    "{}=debug,memory=debug,tower_http=debug,reqwest=debug",
                    env!("CARGO_CRATE_NAME")
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    let synx = Synx::builder()
        .with_db({
            match cli.database {
                Database::Heed { path, regenerate } => {
                    tokio::fs::create_dir_all(&path).await?;
                    if regenerate {
                        tokio::fs::remove_dir_all(&path).await?;
                        tokio::fs::create_dir_all(&path).await?;
                    }

                    let env = unsafe {
                        EnvOpenOptions::new()
                            .map_size(10 * 1024 * 1024 * 1024) // 10 GB
                            .max_dbs(6)
                            .open(path)?
                    };

                    Arc::new(SynxHeedDatabase::new(Arc::new(env), true)?)
                }
                Database::InMemory => Arc::new(SynxInMemory::new()),
            }
        })
        .with_document_embedder(Arc::new(
            VoyageAiEmbedder::builder()
                .model(EmbeddingModel::Voyage3)
                .input_type(EmbeddingInputType::Document)
                .build()?,
        ))
        .with_query_embedder(Arc::new(
            VoyageAiEmbedder::builder()
                .model(EmbeddingModel::Voyage3)
                .input_type(EmbeddingInputType::Query)
                .build()?,
        ))
        .with_summarizer(Arc::new(
            AnthropicCompletion::builder()
                .with_model(Model::ClaudeThreeHaiku)
                .with_temperature(0.0)
                .with_max_tokens(1024)
                .with_system(vec![
                    indoc::indoc! {"
                        You are an AI assistant tasked with summarizing conversations from the user perspective.

                        The summaries you provide will be used to NLP-search, so they should always include comprehensive information regarding the conversation and using an adequate style, easy to search.
                    "}.into()
                ])
                .build()?,
        ))
        .with_executor(Arc::new(TokioExecutor))
        .build();

    let listener = TcpListener::bind((cli.host, cli.port)).await?;
    tracing::debug!("listening on {}", listener.local_addr()?);
    axum::serve(
        listener,
        api::routes::router(synx)
            .route_layer(middleware::from_fn_with_state(
                cli.api_key.into(),
                auth_middleware,
            ))
            .route("/healthz", get(api::handlers::healthz))
            .layer(TraceLayer::new_for_http()),
    )
    .await?;

    Ok(())
}
