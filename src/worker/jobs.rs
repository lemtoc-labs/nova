//! Bounded background job execution.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::cache::CacheKey;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum JobPoolError {
    #[error("job pool is closed")]
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobOutcome<T> {
    Completed(T),
    Panicked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobResult<T> {
    pub generation: u64,
    pub key: CacheKey,
    pub started_at: Instant,
    pub finished_at: Instant,
    pub outcome: JobOutcome<T>,
}

pub struct JobPool<T> {
    sender: Option<mpsc::Sender<Job<T>>>,
    workers: Vec<JoinHandle<()>>,
}

struct Job<T> {
    generation: u64,
    key: CacheKey,
    timeout: Duration,
    run: Box<dyn FnOnce(Instant) -> T + Send + 'static>,
}

impl<T> JobPool<T>
where
    T: Send + 'static,
{
    pub fn new<E>(max_concurrency: usize, results: mpsc::Sender<E>) -> Self
    where
        E: From<JobResult<T>> + Send + 'static,
    {
        let worker_count = max_concurrency.max(1);
        let (sender, receiver) = mpsc::channel::<Job<T>>();
        let receiver = Arc::new(Mutex::new(receiver));
        let workers = (0..worker_count)
            .map(|_index| spawn_worker(Arc::clone(&receiver), results.clone()))
            .collect();

        Self {
            sender: Some(sender),
            workers,
        }
    }

    pub fn spawn<F>(
        &self,
        generation: u64,
        key: CacheKey,
        timeout: Duration,
        run: F,
    ) -> Result<(), JobPoolError>
    where
        F: FnOnce(Instant) -> T + Send + 'static,
    {
        let Some(sender) = &self.sender else {
            return Err(JobPoolError::Closed);
        };

        sender
            .send(Job {
                generation,
                key,
                timeout,
                run: Box::new(run),
            })
            .map_err(|_error| JobPoolError::Closed)
    }
}

impl<T> Drop for JobPool<T> {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn spawn_worker<T, E>(
    receiver: Arc<Mutex<mpsc::Receiver<Job<T>>>>,
    results: mpsc::Sender<E>,
) -> JoinHandle<()>
where
    T: Send + 'static,
    E: From<JobResult<T>> + Send + 'static,
{
    thread::spawn(move || {
        loop {
            let job = {
                let Ok(receiver) = receiver.lock() else {
                    return;
                };
                receiver.recv()
            };

            let Ok(job) = job else {
                return;
            };

            let started_at = Instant::now();
            let deadline = started_at + job.timeout;
            let outcome = match catch_unwind(AssertUnwindSafe(|| (job.run)(deadline))) {
                Ok(value) => JobOutcome::Completed(value),
                Err(_panic) => JobOutcome::Panicked,
            };
            let result = JobResult {
                generation: job.generation,
                key: job.key,
                started_at,
                finished_at: Instant::now(),
                outcome,
            };

            if results.send(result.into()).is_err() {
                return;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(source: &str) -> CacheKey {
        CacheKey::new("git_status", source, 1)
    }

    #[test]
    fn executes_jobs_and_sends_results() -> Result<(), Box<dyn std::error::Error>> {
        let (results, receiver) = mpsc::channel::<JobResult<String>>();
        let pool = JobPool::new(2, results);
        let key = key("/repo");

        pool.spawn(7, key.clone(), Duration::from_secs(1), |_deadline| {
            "done".to_string()
        })?;

        let result = receiver.recv_timeout(Duration::from_secs(1))?;
        assert_eq!(result.generation, 7);
        assert_eq!(result.key, key);
        assert_eq!(result.outcome, JobOutcome::Completed("done".to_string()));
        assert!(result.finished_at >= result.started_at);
        Ok(())
    }

    #[test]
    fn provides_a_deadline_to_jobs() -> Result<(), Box<dyn std::error::Error>> {
        let (results, receiver) = mpsc::channel::<JobResult<bool>>();
        let pool = JobPool::new(1, results);

        pool.spawn(1, key("/repo"), Duration::from_secs(1), |deadline| {
            deadline > Instant::now()
        })?;

        let result = receiver.recv_timeout(Duration::from_secs(1))?;
        assert_eq!(result.outcome, JobOutcome::Completed(true));
        Ok(())
    }

    #[test]
    fn converts_job_results_for_custom_event_channels() -> Result<(), Box<dyn std::error::Error>> {
        #[derive(Debug, PartialEq, Eq)]
        struct TestEvent(JobResult<String>);

        impl From<JobResult<String>> for TestEvent {
            fn from(result: JobResult<String>) -> Self {
                Self(result)
            }
        }

        let (events, receiver) = mpsc::channel::<TestEvent>();
        let pool = JobPool::new(1, events);
        let key = key("/repo");

        pool.spawn(3, key.clone(), Duration::from_secs(1), |_deadline| {
            "converted".to_string()
        })?;

        let event = receiver.recv_timeout(Duration::from_secs(1))?;
        assert_eq!(event.0.generation, 3);
        assert_eq!(event.0.key, key);
        assert_eq!(
            event.0.outcome,
            JobOutcome::Completed("converted".to_string())
        );
        Ok(())
    }

    #[test]
    fn catches_panics_and_keeps_worker_alive() -> Result<(), Box<dyn std::error::Error>> {
        let (results, receiver) = mpsc::channel::<JobResult<String>>();
        let pool = JobPool::new(1, results);

        pool.spawn(
            1,
            key("/panic"),
            Duration::from_secs(1),
            |_deadline| -> String {
                panic!("collector failed");
            },
        )?;
        pool.spawn(2, key("/repo"), Duration::from_secs(1), |_deadline| {
            "recovered".to_string()
        })?;

        let first = receiver.recv_timeout(Duration::from_secs(1))?;
        let second = receiver.recv_timeout(Duration::from_secs(1))?;

        assert_eq!(first.outcome, JobOutcome::Panicked);
        assert_eq!(
            second.outcome,
            JobOutcome::Completed("recovered".to_string())
        );
        Ok(())
    }

    #[test]
    fn respects_max_concurrency_one() -> Result<(), Box<dyn std::error::Error>> {
        let (results, receiver) = mpsc::channel::<JobResult<String>>();
        let pool = JobPool::new(1, results);
        let (first_started, wait_for_release) = mpsc::channel();
        let (release_first, first_release) = mpsc::channel();

        pool.spawn(1, key("/first"), Duration::from_secs(1), move |_deadline| {
            first_started.send(()).expect("start signal should send");
            first_release.recv().expect("release signal should arrive");
            "first".to_string()
        })?;
        pool.spawn(2, key("/second"), Duration::from_secs(1), |_deadline| {
            "second".to_string()
        })?;

        wait_for_release.recv_timeout(Duration::from_secs(1))?;
        assert!(
            receiver.recv_timeout(Duration::from_millis(50)).is_err(),
            "second job should not run while first job is blocked"
        );

        release_first.send(())?;
        let first = receiver.recv_timeout(Duration::from_secs(1))?;
        let second = receiver.recv_timeout(Duration::from_secs(1))?;

        assert_eq!(first.generation, 1);
        assert_eq!(second.generation, 2);
        Ok(())
    }
}
