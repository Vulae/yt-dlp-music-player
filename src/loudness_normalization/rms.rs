
use rodio::Source;



pub fn loudness_normalization_rms<S: Source<Item = i16>>(source: &mut S, loudness_target: f64) -> f64 {
    let mut sum_of_squares: i64 = 0;
    let mut num_samples: u64 = 0;

    // The loading of samples is what takes the majority of the time.
    // But this all should be somewhat unnoticeable with a release build.
    while let Some(sample) = source.next() {
        let sample = sample as i64;
        sum_of_squares += sample * sample;
        num_samples += 1;
    }

    if num_samples == 0 { return 1.0 }

    let rms = ((sum_of_squares as f64) / (num_samples as f64)).sqrt();

    let gain = ((loudness_target / rms) / (source.channels() as f64)) * 10000.0; // ???

    gain
}


