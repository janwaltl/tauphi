pub mod error;
pub mod sampling;
pub mod symbols;

#[tokio::main]
async fn main() {
    let sampler = sampling::Sampler::new_cpu(0, 5).expect("Failed to start the sampling.");
    let sampler = sampling::AsyncSampler::from_sync(sampler).unwrap();
    for i in 1..10 {
        let sample = sampler.get_sample().await.unwrap();
        println!("#{i} {:#?}", sample);
    }
}
