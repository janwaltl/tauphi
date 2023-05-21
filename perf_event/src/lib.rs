use libc::pid_t;
use std::os::fd::AsRawFd;
use std::os::raw::{c_int, c_uchar};
use std::ptr;

use crate::error::PerfError;

pub mod error;

#[repr(C)]
#[derive(Debug)]
pub struct PerfEventHandle {
    fd: c_int,
    perf_buffer: *mut u8,
    perf_buffer_size: usize,
}

impl AsRawFd for PerfEventHandle {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.fd
    }
}

impl Drop for PerfEventHandle {
    fn drop(&mut self) {
        unsafe {
            pe_stop(self);
            pe_close(self);
        }
    }
}

extern "C" {
    fn pe_open_event_sampler(
        cpu: c_int,
        pid: pid_t,
        frequency: usize,
        poll_freq: usize,
        num_pages: usize,
        callchain_depth_limit: usize,
        handle: *mut PerfEventHandle,
    ) -> bool;

    fn pe_close(handle: *mut PerfEventHandle);

    fn pe_start(handle: *const PerfEventHandle, do_reset: bool) -> bool;

    fn pe_stop(handle: *const PerfEventHandle) -> bool;

    fn pe_get_event(
        handle: *const PerfEventHandle,
        dest: *mut c_uchar,
        n: usize,
        peek_only: bool,
    ) -> usize;
}

impl PerfEventHandle {
    /// Open a new perf_event sampler.
    ///
    /// The sampler is created in a stopped move and must be started via
    /// a call to method [Self::start()].
    ///
    /// See `man perf_event_open (2)` for details.
    ///
    /// # Arguments
    ///
    /// * `cpu` Index of CPU to start sampling, -1 to sample all CPUs.
    /// * `pid` Process ID to sample, -1 to sample all processes.
    /// * `frequency` Number of samples per second to generate.
    /// * `poll_freq` How many many samples per POLLIN activation.
    /// * `num_pages` Size of the internal buffer for storing samples,
    ///   in number of pages. Must be a power of two.
    /// * `callchain_depth_limit` Maximum length of the stack trace to record.
    ///
    /// Do note that either `cpu` or `pid` must not be `-1`, one cannot sample
    /// all processes on all CPUs, create an event per-CPU instead.
    pub fn new(
        cpu: c_int,
        pid: pid_t,
        frequency: usize,
        poll_freq: usize,
        num_pages: usize,
        callchain_depth_limit: usize,
    ) -> Result<PerfEventHandle, PerfError> {
        let mut handle = PerfEventHandle {
            fd: 0,
            perf_buffer: ptr::null_mut(),
            perf_buffer_size: 0,
        };
        unsafe {
            if pe_open_event_sampler(
                cpu,
                pid,
                frequency,
                poll_freq,
                num_pages,
                callchain_depth_limit,
                &mut handle,
            ) {
                Ok(handle)
            } else {
                Err(PerfError::FailedOpen)
            }
        }
    }

    /// Start sampling.
    ///
    /// # Arguments
    ///
    /// * `do_reset` Whether to remove all previously collected samples.
    pub fn start(&self, do_reset: bool) -> Result<(), PerfError> {
        unsafe {
            if pe_start(self, do_reset) {
                Ok(())
            } else {
                Err(PerfError::FailedStart)
            }
        }
    }

    /// Stop sample collection.
    pub fn stop(&self) -> Result<(), PerfError> {
        unsafe {
            if pe_stop(self) {
                Ok(())
            } else {
                Err(PerfError::FailedStop)
            }
        }
    }

    /// Extract the next sample from the internal buffer.
    ///
    ///
    /// # Arguments
    ///
    /// * `dest` Buffer to place the sample into.
    /// * `peek_only` Whether to keep the sample in the internal buffer.
    ///   If true, the next call will return the same sample.
    ///
    /// # Returns
    ///
    /// True size of the sample.
    pub fn get_event(&self, dest: &mut [u8], peek_only: bool) -> usize {
        unsafe { pe_get_event(self, dest.as_mut_ptr(), dest.len(), peek_only) }
    }
}
