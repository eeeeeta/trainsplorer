//! Background worker to handle database updating.
//!
//! Currently not the most intelligent piece of software, but it works (hopefully).

use tspl_sqlite::TsplPool;
use crate::download::{Downloader, JobType};
use crate::config::Config;
use chrono::*;
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;
use log::*;
use tspl_sqlite::traits::*;
use crate::types::ScheduleFile;
use crate::errors::Result;

pub type DlSender = Sender<JobType>;

/// The hour of the day when schedule update jobs are started (default value)
static UPDATE_HOUR: u32 = 5;
/// How many milliseconds to wait before retrying a failed update job.
/// (default value)
///
/// This value is doubled for every failure (yay, exponential backoff!)
static UPDATE_TIMEOUT_MS: u32 = 16000;
/// How many retries per update job (default value)
static UPDATE_RETRIES: u32 = 9;

pub struct UpdateScheduler {
    pool: TsplPool,
    dl: Downloader,
    rx: Receiver<JobType>,
    tx: Option<Sender<JobType>>,
    last_update: DateTime<Local>,
    update_time: NaiveTime,
    update_timeout_ms: u32,
    update_retries: u32
}
impl UpdateScheduler {
    pub fn new(pool: TsplPool, cfg: &Config) -> Result<Self> {
        let dl = Downloader::new(cfg);
        let (tx, rx) = mpsc::channel();
        let update_hour = cfg.update_hour.unwrap_or(UPDATE_HOUR);
        let update_time = NaiveTime::from_hms(update_hour, 0, 0);

        // To find the last update, we grab the last inserted ScheduleFile
        // from the DB and use its timestamp, in order to ensure updates
        // happen even if tspl-fahrplan dies mid-update.

        let last_update = {
            let mut conn = pool.get().unwrap();
            let sf = ScheduleFile::from_select(&mut conn, "ORDER BY timestamp DESC LIMIT 1", NO_PARAMS)?;
            if let Some(sf) = sf.into_iter().nth(0) {
                Local.timestamp(sf.timestamp as _, 0)
            }
            else {
                Local::now()
            }
        };
        info!("Last schedule update job ran at: {}", last_update);
        Ok(Self { 
            pool, dl, rx, 
            tx: Some(tx),
            last_update,
            update_time,
            update_timeout_ms: cfg.update_timeout_ms.unwrap_or(UPDATE_TIMEOUT_MS),
            update_retries: cfg.update_retries.unwrap_or(UPDATE_RETRIES),
        })
    }
    pub fn take_sender(&mut self) -> Sender<JobType> {
        self.tx.take().unwrap()
    }
    pub fn update(&mut self) {
        let mut cur = 0;
        let mut timeout_ms = self.update_timeout_ms;
        loop {
            info!("Attempting to download update (try #{})", cur+1); 
            let mut conn = self.pool.get().unwrap();
            match self.dl.do_update(&mut conn) {
                Ok(_) => {
                    info!("Update successfully downloaded.");
                    break;
                },
                Err(e) => {
                    warn!("Failed to fetch update: {}", e);
                    cur += 1;
                    if cur > self.update_retries {
                        error!("Attempt to fetch timed out! Recovery required.");
                        break;
                    }
                    warn!("Retrying in {} ms...", timeout_ms);
                    thread::sleep(::std::time::Duration::from_millis(timeout_ms as _));
                    timeout_ms *= 2;
                }
            }
        }

    }
    pub fn run(mut self) -> Result<()> {
        info!("Running update job scheduler");
        thread::Builder::new()
            .name("tspl-fahrplan: update scheduler".into())
            .spawn(move || {
                loop {
                    let now = Local::now();
                    if now.date() != self.last_update.date() && now.time() >= self.update_time {
                        info!("Schedule update job triggered (last update: {})", self.last_update);
                        self.update();
                        self.last_update = now;
                    }
                    if let Ok(j) = self.rx.try_recv() {
                        info!("Performing manual job: {:?}", j);
                        let mut conn = self.pool.get().unwrap();
                        match self.dl.do_job(&mut conn, j) {
                            Ok(_) => info!("Job completed successfully."),
                            Err(e) => error!("Manual job failed: {}", e)
                        }
                    }
                    thread::sleep(::std::time::Duration::from_millis(1000));
                }
            })?;
        Ok(())
    }
}
