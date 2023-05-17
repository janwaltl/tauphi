use crate::pe::*;
use std::mem;
use std::os::raw::c_uchar;
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

pub struct CpuSampler {
    handle: PerfEventHandle,
}

impl CpuSampler {
    pub fn new(cpu: usize, frequency: usize) -> CpuSampler {
        let mut handle = PerfEventHandle {
            fd: 0,
            perf_buffer: ptr::null_mut(),
            perf_buffer_size: 0,
        };
        let sample_size = mem::size_of::<CpuSample>();
        // Store roughly 10 seconds of events.
        // perf_event requires the size to be a power of two.
        // Assume 4KB pages right now.
        let num_pages = (10 * frequency * sample_size / 4096 + 1).next_power_of_two();
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
}

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

impl Iterator for CpuSampler {
    type Item = CpuSample;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.get_sample() {
                None => (),
                x => return x,
            }
        }
    }
}
