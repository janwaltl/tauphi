//! Sampling of CPUs or processes based leveraging Linux perf events.
use crate::error::PerfError;
use crate::pe::*;
use libc;
use std::mem;
use std::os::raw::c_uchar;
use std::ptr;

/// A collected sample.
///
/// Layout-complatible with the raw perf_event sample.
#[repr(C)]
#[derive(Default, Debug)]
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
    /// Padding
    pub cpu_pad: u32,
}

/// Asynchronous sampling of a single CPU or PID.
///
/// The sampling starts on `new()` and ends when `drop()` is called.
/// The samples are collected in an internal buffer and `get_sample()`
/// is used to retrieve them. If the internal buffers overflows,
/// samples will be discarded.
///
/// # Examples
/// ```no_run
/// use perf_event::sampling::Sampler;
/// let pid = 12; // PID of the process to sample.
/// let mut sampler = Sampler::new_pid(pid,10);
/// // Samples are now being collected by the Linux kernel.
/// loop{
///     if let Some(sample) =  sampler.get_sample() {
///         println!("Sample: {:?}", sample);
///     }
/// }
/// drop(sampler); // Stop collecting the samples.
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
        assert!(num_pages > 0);
        unsafe {
            if !pe_open_event_sampler(cpu as i32, pid as i32, frequency, num_pages, &mut handle) {
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
        let expected_size = mem::size_of::<Sample>();

        unsafe {
            let mut sample = Sample::default();
            let sample_size = pe_get_event(
                &self.handle,
                (&mut sample as *mut Sample) as *mut c_uchar,
                expected_size,
                false,
            );
            if sample_size == expected_size {
                Some(sample)
            } else {
                None
            }
        }
    }

    /// Store at least X seconds of pending samples in the internal perf buffer.
    const BUFFER_SIZE_SECS: usize = 10;
}

/// Infinite iterator over the gathered samples.
///
/// `next()` blocks if necessary to wait for the next sample.
impl Iterator for Sampler {
    type Item = Sample;

    /// Returns the next sample, blocks until there is one.
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.get_sample() {
                None => (),
                x => return x,
            }
        }
    }
}
