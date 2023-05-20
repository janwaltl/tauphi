mod pe {
    use libc::pid_t;
    use std::os::fd::AsRawFd;
    use std::os::raw::{c_int, c_uchar};
    #[repr(C)]
    #[derive(Debug)]
    pub struct PerfEventHandle {
        pub fd: c_int,
        pub perf_buffer: *mut u8,
        pub perf_buffer_size: usize,
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
        /// Open a new perf_event sampler.
        ///
        /// The sampler starts in a stopped move and must be started via
        /// a call to `pe_start()`.
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
        /// * `handle` Handle to initialize the sample.
        ///
        /// Do note that either `cpu` or `pid` must not be `-1`, one cannot sample all processes on all
        /// CPUs, create an event per-CPU instead.
        ///
        ///
        /// # Returns
        ///
        /// Whether the new handle has been initialized.
        pub fn pe_open_event_sampler(
            cpu: c_int,
            pid: pid_t,
            frequency: usize,
            poll_freq: usize,
            num_pages: usize,
            handle: *mut PerfEventHandle,
        ) -> bool;

        /// Close the perf_event handle.
        /// Safe to call on even on uninitialized handles.
        ///
        /// # Arguments
        /// * `handle` Handle to close, can be NULL.
        pub fn pe_close(handle: *mut PerfEventHandle);

        /// Start sampling.
        ///
        /// # Arguments
        ///
        /// * `handle` Handle to opened perf_event.
        /// * `do_reset` Whether to remove all previously collected samples.
        ///
        /// # Returns
        ///
        /// Whether the sampling began.
        pub fn pe_start(handle: *const PerfEventHandle, do_reset: bool) -> bool;

        /// Stop sample collection.
        ///
        /// # Arguments
        ///
        /// * `handle` Handle to the opened perf_event.
        ///
        /// Whether the sampling has been stopped.
        pub fn pe_stop(handle: *const PerfEventHandle) -> bool;

        /// Extract the next sample from the internal buffer.
        ///
        ///
        /// # Arguments
        ///
        /// * `handle` Handle to the opened perf_event.
        /// * `dest` Buffer to place the sample into.
        /// * `n` Size of the `dest`, at most `n` bytes of the sample will
        ///   be copied.
        /// * `peek_only` Whether to keep the sample in the internal buffer.
        ///   If true, the next call will return the same sample.
        ///
        /// # Returns
        ///
        /// True size of the sample.
        pub fn pe_get_event(
            handle: *const PerfEventHandle,
            dest: *mut c_uchar,
            n: usize,
            peek_only: bool,
        ) -> usize;
    }
}

pub mod error;
pub mod sampling;
