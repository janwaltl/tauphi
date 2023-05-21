use std::io;

use thiserror::Error;

use perf_event as pe;

#[derive(Error, Debug)]
pub enum TauphiError {
    #[error("perf_event failed.")]
    Perf(#[from] pe::error::PerfError),
    #[error("IO error")]
    IO(#[from] io::Error),
}
