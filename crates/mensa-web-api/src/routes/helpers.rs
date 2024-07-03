use std::{future::Future, pin::Pin};

use mongodb::options::ClientOptions;
use tokio_cron_scheduler::{JobScheduler, JobSchedulerError};

use crate::config::DbConfig;

pub trait Pinable: Sized {
    fn pin(self) -> Pin<Box<Self>> { Box::pin(self) }
}

impl<F: Future> Pinable for F {}

pub async fn register_jobs<'a, F>(
    reg: impl FnOnce(JobScheduler) -> F + 'a,
) where F: Future<Output = Result<JobScheduler, JobSchedulerError>>
{
    if let Err(err) = async {
        tracing::info!("starting chron job");
        let shed = JobScheduler::new().await?;
        let shed = reg(shed).await?;
        shed.start().await?;

        tracing::info!("started chron job");
        Ok::<_, JobSchedulerError>(())
    }.await {
        tracing::error!("could not start chron job: {err}");
    }
}

pub async fn connect_db(cfg: &DbConfig) -> Option<mongodb::Database> {
    match async {
        mongodb::Client::with_options(
            ClientOptions::parse(&cfg.url).await?,
        )
    }.await {
        Ok(v) => {
            Some(v.database(&cfg.database))
        },
        Err(err) => {
            tracing::error!("could not connect to db: {err}");
            None
        },
    }
}


