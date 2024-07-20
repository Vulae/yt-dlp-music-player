
mod rms;

use rms::loudness_normalization_rms;
use rodio::Source;



#[derive(Debug, Clone, Copy)]
pub enum LoudnessNormalization {
    None,
    RMS,
    // TODO: EBU R128 loudness normalization procedure
    // https://github.com/FFmpeg/FFmpeg/blob/master/libavfilter/af_loudnorm.c
    EbuR128,
}

impl LoudnessNormalization {
    /// This may or may not consume the source, so you may have to either clone the source, or seek back to wherever the source was at.
    /// This will start where the source is currently at. So all previous samples in source are ignored.
    pub fn get_normal_amplification<S: Source<Item = i16>>(&self, source: &mut S) -> f64 {
        match self {
            LoudnessNormalization::None => 1.0,
            LoudnessNormalization::RMS => loudness_normalization_rms(source, 1.0),
            LoudnessNormalization::EbuR128 => unimplemented!("LoudnessNormalization::EbuR128 N.Y.I."),
        }
    }
}


