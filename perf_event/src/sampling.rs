//! Sampling of CPUs or processes based leveraging Linux perf events.
use crate::pe::*;
use libc;
use std::mem;
use std::os::raw::c_uchar;
use std::ptr;

/// CPU sample
/// layout-complatible with the raw perf_event sample.
#[repr(C)]
#[derive(Default, Debug)]
pub struct CpuSample {
    /// Instruction pointer
    pub ip: u64,
    /// Process ID
    pub pid: u32,
    /// Thread ID
    pub tid: u32,
    /// Monotonic timestamp of this sample.
    pub time: u64,
    /// Sampled CPU
    pub cpu: u32,
    /// Padding
    pub cpu_pad: u32,
}

/// Asynchronous sampling of a single CPU.
///
/// The sampling starts on `new()` and ends when `drop()` is called.
/// The samples are collected in an internal buffer and `get_sample()`
/// is used to retrieve them. If the internal buffers overflows,
/// samples will be discarded.
///
/// # Examples
/// ```no_run
/// use perf_event::sampling::CpuSampler;
/// let mut sampler = CpuSampler::new(0,10);
/// // Samples are now being collected by the Linux kernel.
/// loop{
///     if let Some(sample) =  sampler.get_sample() {
///         println!("Sample: {:?}", sample),
///     }
/// }
/// drop(sampler); // Stop collecting the samples.
/// ```
pub struct CpuSampler {
    handle: PerfEventHandle,
}

impl CpuSampler {
    /// Start a new sampler for the required CPU at given frequency.
    ///
    /// # Arguments
    /// * `cpu` CPU to periodically sample, indexed from 0 to number of CPUs.
    /// * `frequency` how many samples per second to generate.
    pub fn new(cpu: usize, frequency: usize) -> CpuSampler {
        let mut handle = PerfEventHandle {
            fd: 0,
            perf_buffer: ptr::null_mut(),
            perf_buffer_size: 0,
        };
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGE_SIZE) as usize };

        let sample_size = mem::size_of::<CpuSample>();
        // Store at least X seconds of events.
        // perf_event requires the size to be a power of two.
        // That also handles the case of 0->1 pages to integer division.
        let num_pages =
            (Self::BUFFER_SIZE_SECS * frequency * sample_size / page_size).next_power_of_two();
        assert!(num_pages > 0);
        unsafe {
            if !pe_open_cpu_sample(cpu, frequency, num_pages, &mut handle) {
                panic!("Failed to open the sampler.");
            }
            if !pe_start(&handle, true) {
                panic!("Failed to start the sampler.");
            }
        }
        CpuSampler { handle }
    }

    /// Return the next sample if there is one available.
    fn get_sample(self: &Self) -> Option<CpuSample> {
        let expected_size = mem::size_of::<CpuSample>();

        unsafe {
            let mut sample = CpuSample::default();
            let sample_size = pe_get_event(
                &self.handle,
                (&mut sample as *mut CpuSample) as *mut c_uchar,
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

/// Close the opened underlying perf_event sampler.
impl Drop for CpuSampler {
    fn drop(&mut self) {
        unsafe {
            if !pe_stop(&self.handle) {
                panic!("Failed to stop the sampler.");
            }
            pe_close(&mut self.handle);
        }
    }
}

/// Infinite iterator over the gathered samples.
///
/// `next()` blocks if necessary to wait for the next sample.
impl Iterator for CpuSampler {
    type Item = CpuSample;

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
