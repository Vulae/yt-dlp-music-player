
use crate::song::Song;



#[derive(Debug)]
pub struct Playlist {
    songs: Vec<Song>,
    song_index: usize,
}

impl Playlist {
    pub fn new(songs: Vec<Song>) -> Playlist {
        Playlist { songs, song_index: 0 }
    }

    pub fn seek(&mut self, offset: isize) -> Option<Song> {
        self.song_index = self.song_index.checked_add_signed(offset)? % self.songs.len();
        self.songs.get(self.song_index).cloned()
    }
}


