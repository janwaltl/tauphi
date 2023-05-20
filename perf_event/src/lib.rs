mod pe {
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
        pub fn pe_open_cpu_sample(
            cpu: usize,
            frequency: usize,
            num_pages: usize,
            handle: *mut PerfEventHandle,
        ) -> bool;
        pub fn pe_close(handle: *mut PerfEventHandle);

        pub fn pe_start(handle: *const PerfEventHandle, do_reset: bool) -> bool;
        pub fn pe_stop(handle: *const PerfEventHandle) -> bool;
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
