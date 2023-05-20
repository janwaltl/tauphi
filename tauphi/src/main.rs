use perf_event::sampling;
fn main() {
    let sampler = sampling::Sampler::new_cpu(0, 100).expect("Failed to start the sampling.");

    for val in sampler.take(10) {
        println!("Value: {:#?}", val);
    }

    println!("Hello, world!");
}
