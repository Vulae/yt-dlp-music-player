
use std::{fs, io::{BufReader, Cursor}, path::PathBuf, time::Duration};
use anyhow::{anyhow, Result};
use rodio::{Sink, Source};



#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum LoudnessNormalization {
    None,
    RMS,
    // TODO: EBU R128 loudness normalization procedure
}

const LOUDNESS_TARGET: f64 = 1.0;

impl LoudnessNormalization {
    /// This may or may not consume the source, so you may have to either clone the source, or seek back to wherever the source was at.
    /// This will start where the source is currently at. So all previous samples in source are ignored.
    pub fn get_normal_amplification<S: Source<Item = i16>>(&self, source: &mut S) -> f64 {
        match self {
            LoudnessNormalization::None => LOUDNESS_TARGET,
            LoudnessNormalization::RMS => {
                let mut suq: i64 = 0;
                let mut num_samples: u64 = 0;

                // The loading of samples is what takes the majority of the time.
                // But this all should be somewhat unnoticeable with a release build.
                while let Some(sample) = source.next() {
                    let sample = sample as i64;
                    suq += sample * sample;
                    num_samples += 1;
                }

                if num_samples == 0 { return 1.0 }

                let rms = ((suq as f64) / (num_samples as f64)).sqrt();

                let gain = ((LOUDNESS_TARGET / rms) / (source.channels() as f64)) * 10000.0; // ???

                gain
            },
        }
    }
}



#[derive(Debug, Clone)]
pub struct Song {
    file: PathBuf,
}

impl Song {
    pub fn load_playlist_directory(playlist_directory: &PathBuf) -> Result<Vec<Song>> {
        let mut songs: Vec<Song> = vec![];

        let binding = fs::read_dir(playlist_directory)?
            .collect::<Result<Vec<_>, _>>()?;
        let song_files = binding
            .iter()
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".m4a"))
            .collect::<Vec<_>>();

        for song_file in song_files {
            songs.push(Song {
                file: song_file.path()
            });
        }

        Ok(songs)
    }

    pub fn sink_load(&self, sink: &mut Sink, loudness_normalization: bool) -> Result<Duration> {
        
        // Loading the whole file to prevent stuttering.
        // Pretty sure this *should* be fine as audio data shouldn't really be > 50MiB.
        let data = fs::read(&self.file)?;
        let mut source = rodio::Decoder::new(BufReader::new(Cursor::new(data)))?;
        // Could probably use rodio::Buffered, but that may add a delay on audio controls. I don't really know though, haven't tested it yet.

        let duration = if let Some(duration) = source.total_duration() {
            duration
        } else {
            return Err(anyhow!("Failed to get song duration."));
        };

        if loudness_normalization {
            let amplify_amount = LoudnessNormalization::RMS.get_normal_amplification(&mut source);
            source.try_seek(Duration::ZERO).unwrap(); // FIXME: ? instead of .unwrap()
            let normalized_source = source.amplify(amplify_amount as f32);
            sink.append(normalized_source);
        } else {
            sink.append(source);
        }

        Ok(duration)
    }

    pub fn name(&self) -> String {
        let filename = self.file.file_name().unwrap().to_string_lossy();
        let without_id = filename.split_at(filename.find(' ').unwrap() + 1).1;
        let without_ext = &without_id[0..without_id.len() - 4];
        without_ext.to_string()
    }
}


