use thiserror::Error;

#[derive(Error, Debug)]
pub enum PerfError {
    #[error("perf_event could not be opened.")]
    FailedOpen,
    #[error("perf_event could not be started.")]
    FailedStart,
}
