use std::os::raw::c_int;

#[repr(C)]
#[derive(Debug)]
struct PerfEventHandle {
    fd: c_int,
    perf_buffer: *mut u8,
    perf_buffer_size: usize,
}

extern "C" {
    fn pe_open_cpu_sample(
        cpu: usize,
        frequency: usize,
        num_pages: usize,
        handle: *mut PerfEventHandle,
    ) -> bool;
    fn pe_close(handle: *mut PerfEventHandle);

    fn pe_start(handle: *const PerfEventHandle, do_reset: bool) -> bool;
    fn pe_stop(handle: *const PerfEventHandle) -> bool;
    fn pe_get_event(
        handle: *const PerfEventHandle,
        dest: *mut u8,
        n: usize,
        peek_only: bool,
    ) -> usize;
}

pub mod sampling {
    use super::*;
    use std::mem;
    use std::ptr;

    #[repr(C)]
    #[derive(Default, Debug)]
    pub struct CpuSample {
        ip: u64,
        pid: u32,
        tid: u32,
        time: u64,
        cpu: u32,
        cpu_pad: u32,
    }

    pub fn sample_cpu() {
        let mut handle = PerfEventHandle {
            fd: 0,
            perf_buffer: ptr::null_mut(),
            perf_buffer_size: 0,
        };
        unsafe {
            if !pe_open_cpu_sample(0, 10, 8, &mut handle) {
                panic!("Failed to open the sampler.");
            }
            println!("Handle {:?}", handle);
            if !pe_start(&handle, true) {
                panic!("Failed to start the sampler.");
            }
            use std::{thread, time};
            thread::sleep(time::Duration::from_millis(1000));

            let mut sample = RecordSample::default();

            loop {
                let sample_size = pe_get_event(
                    &handle,
                    (&mut sample as *mut RecordSample) as *mut u8,
                    mem::size_of::<RecordSample>(),
                    false,
                );
                println!("Sample size: {sample_size}");
                if sample_size == 0 {
                    break;
                }
            }

            if !pe_stop(&handle) {
                panic!("Failed to stop the sampler.");
            }
            pe_close(&mut handle);
        }
    }
}
