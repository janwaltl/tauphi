use perf_event::sampling;
fn main() {
    sampling::safe_foo();
    println!("Hello, world!");
}
