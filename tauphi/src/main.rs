use perf_event::sampling;
fn main() {
    let sampler = sampling::CpuSampler::new(0, 100);

    for val in sampler.take(10) {
        println!("Value: {:#?}", val);
    }

    println!("Hello, world!");
}
