use perf_event::sampling;
fn main() {
    sampling::sample_cpu();
    println!("Hello, world!");
}
