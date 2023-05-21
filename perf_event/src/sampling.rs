//! Sampling of CPUs or processes based leveraging Linux perf events.
use std::mem;
use std::os::{fd::AsRawFd, raw::c_uchar};
use std::ptr;
use std::thread;

use libc;
use tokio::io::unix::AsyncFd;

use crate::{error::PerfError, pe::*};

/// Maximum entries in the stack trace.
///
/// 123 ensures that the raw sample is 1KB in size.
const CALLCHAIN_DEPTH: usize = 123;

/// A collected sample.
#[derive(Debug, Default)]
pub struct Sample {
    /// Instruction pointer
    pub ip: u64,
    /// Process ID
    pub pid: u32,
    /// Thread ID
    pub tid: u32,
    /// Timestamp of this sample in nanoseconds, monotonic
    pub time: u64,
    /// Sampled CPU index
    pub cpu: u32,
    /// Instruction pointers for the callchain.
    pub callchain: Vec<u64>,
}

/// Layout-complatible with the raw perf_event sample.
#[repr(C)]
#[derive(Debug)]
struct RawSample {
    /// Instruction pointer
    ip: u64,
    /// Process ID
    pid: u32,
    /// Thread ID
    tid: u32,
    /// Timestamp of this sample in nanoseconds, monotonic
    time: u64,
    /// Sampled CPU index
    cpu: u32,
    /// Padding
    cpu_pad: u32,
    /// Number of callchain entries in callchain array.
    callchain_entries: u64,
    /// Calchain instruction pointers.
    callchain: [u64; CALLCHAIN_DEPTH],
}

impl Default for RawSample {
    fn default() -> Self {
        RawSample {
            ip: 0,
            pid: 0,
            tid: 0,
            time: 0,
            cpu: 0,
            cpu_pad: 0,
            callchain_entries: 0,
            callchain: [0; CALLCHAIN_DEPTH],
        }
    }
}

/// Asynchronous sampling of a single CPU or PID.
///
/// Offers synchronous API, for fully asynchronous variant, see [AsyncSampler].
///
/// The sampling starts on construction and ends when [drop()] is called.
/// The samples are collected in an internal buffer and [Sampler::get_sample()]
/// is used to retrieve them. If the internal buffers overflows,
/// samples will be discarded.
///
/// # Examples
/// ```no_run
/// use perf_event::sampling::Sampler;
/// let pid = 12; // PID of the process to sample.
/// let mut sampler = Sampler::new_pid(pid,10).expect("Failed to start the sampling");
/// // Samples are now being collected by the Linux kernel.
/// // Use blocking iterator to access them.
/// for sample in sampler.take(10) {
///     println!("Sample: {:#?}", sample);
/// }
/// ```
pub struct Sampler {
    handle: PerfEventHandle,
}

impl Sampler {
    /// Start a new sampler for the required CPU at given frequency.
    ///
    /// # Arguments
    /// * `cpu` CPU to periodically sample, indexed from 0 to number of CPUs.
    /// * `frequency` how many samples per second to generate.
    pub fn new_cpu(cpu: i32, frequency: usize) -> Result<Sampler, PerfError> {
        Self::new(cpu, -1, frequency)
    }

    /// Start a new sampler of the given process at the given frequency.
    ///
    /// Do note this does not perform off-cpu sampling, if the process is
    /// not running (for any reason), the samples are not collected.
    ///
    /// # Arguments
    /// * `pid` Process with ID to periodically sample.
    /// * `frequency` how many samples per second to generate.
    pub fn new_pid(pid: i32, frequency: usize) -> Result<Sampler, PerfError> {
        Self::new(-1, pid, frequency)
    }

    /// Wrapper around pe_open_event_sampler()
    fn new(cpu: i32, pid: i32, frequency: usize) -> Result<Sampler, PerfError> {
        let mut handle = PerfEventHandle {
            fd: 0,
            perf_buffer: ptr::null_mut(),
            perf_buffer_size: 0,
        };
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGE_SIZE) as usize };

        let sample_size = mem::size_of::<Sample>();
        // Store at least X seconds of events.
        // perf_event requires the size to be a power of two.
        // That also handles the case of 0->1 pages due to integer division.
        let num_pages =
            (Self::BUFFER_SIZE_SECS * frequency * sample_size / page_size).next_power_of_two();
        // Target poll every 100ms
        let poll_freq: usize = 1.max(frequency / (1000 / Self::POLL_FREQUENCY_MS));
        assert!(num_pages > 0);
        unsafe {
            if !pe_open_event_sampler(
                cpu as i32,
                pid as i32,
                frequency,
                poll_freq,
                num_pages,
                CALLCHAIN_DEPTH,
                &mut handle,
            ) {
                return Err(PerfError::FailedOpen);
            }
            if !pe_start(&handle, true) {
                return Err(PerfError::FailedStart);
            }
        }
        Ok(Sampler { handle })
    }

    /// Return the next sample if there is one available.
    pub fn get_sample(self: &Self) -> Option<Sample> {
        /// Size of the fixed part of RawSample - without the trailing callchain.
        const FIXED_HEADER_SIZE: usize = mem::size_of::<RawSample>() - 8 * CALLCHAIN_DEPTH;

        unsafe {
            let mut raw_sample = RawSample::default();
            let sample_size = pe_get_event(
                &self.handle,
                (&mut raw_sample as *mut RawSample) as *mut c_uchar,
                mem::size_of::<RawSample>(),
                false,
            );
            if sample_size >= FIXED_HEADER_SIZE {
                return Some(Sample {
                    ip: raw_sample.ip,
                    pid: raw_sample.pid,
                    tid: raw_sample.tid,
                    time: raw_sample.time,
                    cpu: raw_sample.cpu,
                    callchain: raw_sample.callchain[0..raw_sample.callchain_entries as usize]
                        .to_vec(),
                });
            } else {
                return None;
            }
        }
    }

    /// How often is POLLIN triggered on the sampler.
    const POLL_FREQUENCY_MS: usize = 100;
    /// Store at least X seconds of pending samples in the internal perf buffer.
    const BUFFER_SIZE_SECS: usize = 10;
}

/// Expose the raw perf_event file descriptor.
impl AsRawFd for Sampler {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.handle.as_raw_fd()
    }
}

/// Infinite iterator over the gathered samples.
///
/// [Sampler::next()] blocks if necessary to wait for the next sample.
impl Iterator for Sampler {
    type Item = Sample;

    /// Returns the next sample.
    ///
    /// Contains a busy loop, blocking until the sample is available.
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.get_sample() {
                None => (),
                x => return x,
            }
            thread::yield_now();
        }
    }
}

/// Sampler with asynchronous API.
///
/// See [Sampler] for synchronous, iterator-based variant.
///
/// # Examples
/// ```no_run
/// async fn async_main() {
///     use perf_event::sampling::{Sampler,AsyncSampler};
///     let sampler = Sampler::new_cpu(0, 5).expect("Failed to start the sampling.");
///     let sampler = AsyncSampler::from_sync(sampler).unwrap();
///     for i in 1..10 {
///         let sample = sampler.get_sample().await.unwrap();
///         println!("#{i} {:#?}", sample);
///     }
///     drop(sampler); // Stop collecting the samples.
/// }
/// ```
pub struct AsyncSampler {
    poll_fd: AsyncFd<Sampler>,
}

impl AsyncSampler {
    /// Construct an asynchronous version of the Sampler.
    pub fn from_sync(sampler: Sampler) -> Result<AsyncSampler, PerfError> {
        Ok(AsyncSampler {
            poll_fd: AsyncFd::new(sampler)?,
        })
    }

    /// Return the next sample.
    pub async fn get_sample(self: &Self) -> Result<Sample, PerfError> {
        loop {
            // Try to get the sample from the ring buffer, non-blocking.
            if let Some(sample) = self.poll_fd.get_ref().get_sample() {
                return Ok(sample);
            }

            let mut guard = self.poll_fd.readable().await?;
            // Clear the POLLIN flag immedietely. There is no actual read to do,
            // perf_event only signals POLLIN once each time the wakeup counter
            // overflows.
            guard.clear_ready();
        }
    }
}

#[test]
fn raw_sample_alignment_test() {
    assert_eq!(
        1024,
        mem::size_of::<RawSample>(),
        "Ensure that size of the raw sample is nice."
    );
}
