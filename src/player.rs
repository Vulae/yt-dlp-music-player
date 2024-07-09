
use std::{error::Error, fs, io::{BufReader, Cursor}, path::PathBuf, time::Duration};
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};



#[derive(Debug, Clone)]
pub struct Song {
    file: PathBuf,
}

impl Song {
    pub fn load_playlist_directory(playlist_directory: &PathBuf) -> Result<Vec<Song>, Box<dyn Error>> {
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

    pub fn sink_load(&self, sink: &mut Sink) -> Result<Duration, Box<dyn Error>> {
        
        // Loading the whole file to prevent stuttering.
        // Pretty sure this *should* be fine as audio data shouldn't really be > 50MiB.
        let data = fs::read(&self.file)?;
        let source = rodio::Decoder::new(BufReader::new(Cursor::new(data)))?;
        // Could probably use rodio::Buffered, but that may add a delay on audio controls. I don't really know though, haven't tested it yet.

        let duration = source.total_duration().expect("Failed to get song duration");

        sink.append(source);

        Ok(duration)
    }

    pub fn name(&self) -> String {
        self.file.file_name().unwrap().to_string_lossy().to_string()
    }
}



pub struct Player {
    songs: Vec<Song>,
    index: usize,

    _stream: OutputStream,
    _handle: OutputStreamHandle,
    sink: Sink,

    current_song_duration: Duration,
}

#[allow(dead_code)]
impl Player {

    pub fn new(songs: Vec<Song>) -> Result<Player, Box<dyn Error>> {
        let (stream, handle) = rodio::OutputStream::try_default()?;
        let sink = Sink::try_new(&handle)?;

        Ok(Player {
            songs,
            index: 0,
            
            _stream: stream,
            _handle: handle,
            sink,

            current_song_duration: Duration::ZERO,
        })
    }



    pub fn current_song(&self) -> Song { self.songs[self.index].clone() }
    pub fn current_song_duration(&self) -> Duration { self.current_song_duration }
    pub fn current_song_time(&self) -> Duration { unimplemented!() }

    fn load_song(&mut self) {
        self.sink.clear();
        let song = self.current_song();
        self.current_song_duration = song.sink_load(&mut self.sink).expect("Failed to load song.");
    }

    pub fn next_song(&mut self) {
        self.index += 1;
        self.index %= self.songs.len();
        self.load_song();
    }

    pub fn prev_song(&mut self) {
        self.index -= 1;
        if self.index >= self.songs.len() { self.index = 0; }
        self.load_song();
    }



    pub fn play(&mut self) {
        self.sink.play();
    }
    pub fn pause(&mut self) {
        self.sink.pause();
    }
    pub fn stop(&mut self) {
        self.sink.stop();
    }

    pub fn is_playing(&self) -> bool { !self.sink.is_paused() }
    // pub fn is_finished(&self) -> bool { self.current_song_time() >= self.current_song_duration() }
    pub fn is_finished(&self) -> bool { self.sink.empty() } // FIXME: Use above.

    pub fn volume(&self) -> f32 { self.sink.volume() }
    pub fn set_volume(&mut self, volume: f32) { self.sink.set_volume(volume); }

}


