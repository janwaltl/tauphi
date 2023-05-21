use perf_event::sampling;

fn sync_main() {
    let sampler = sampling::Sampler::new_cpu(0, 100).expect("Failed to start the sampling.");

    for val in sampler.take(10) {
        println!("Value: {:#?}", val);
    }

    println!("Hello, world!");
}

async fn async_main() {
    let sampler = sampling::Sampler::new_cpu(0, 5).expect("Failed to start the sampling.");
    let sampler = sampling::AsyncSampler::from_sync(sampler).unwrap();
    for i in 1..10 {
        let sample = sampler.get_sample().await.unwrap();
        println!("#{i} {:#?}", sample);
    }
}

#[tokio::main]
async fn main() {
    async_main().await;
}
