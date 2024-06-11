use rodio::{
    cpal::traits::StreamTrait,
    cpal::{default_host, traits::HostTrait, InputCallbackInfo, SampleFormat, StreamError},
    DeviceTrait,
};

fn get_input_device() -> rodio::Device {
    default_host()
        .default_input_device()
        .expect("found no default input")
}

fn on_input_data(sample: &[i16], _: &InputCallbackInfo) {
    println!("got {} samples", sample.len());
}

fn on_error(err: StreamError) {
    println!("got error: {}", err);
}

fn main() {
    let dev = get_input_device();
    let config = dev
        .supported_input_configs()
        .expect("no supported input configs")
        .find(|cfg| cfg.channels() == 1 && cfg.sample_format() == SampleFormat::I16)
        .expect("no desirable input configs")
        .with_sample_rate(rodio::cpal::SampleRate(16000))
        .config();
    let instream = dev
        .build_input_stream(&config, on_input_data, on_error, None)
        .unwrap();
    println!("starting input stream...");
    instream.play().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(10));
    println!("done!");
}
