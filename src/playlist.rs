
use std::ops::Div;
use rand::prelude::IteratorRandom;
use crate::song::Song;



#[allow(dead_code)]
pub trait PlaylistSeekable {
    fn seek(&mut self, offset: isize) -> Option<Song>;
    fn peek(&mut self, offset: isize) -> Option<Song> {
        if offset == 0 {
            self.seek(0)
        } else {
            let song = self.seek(offset);
            self.seek(-offset);
            song
        }
    }
    fn current(&mut self) -> Option<Song> {
        self.peek(0)
    }
}



#[allow(dead_code)]
#[derive(Debug)]
enum PlaylistShuffle {
    Normal,
    Random,
    SmartRandom {
        blacklist_length: usize,
    },
}



#[derive(Debug)]
pub struct Playlist {
    mode: PlaylistShuffle,
    songs: Vec<Song>,
    song_indices: Vec<usize>,
    song_indices_index: usize,
}

impl Playlist {
    pub fn new(songs: Vec<Song>) -> Playlist {
        Playlist {
            mode: PlaylistShuffle::SmartRandom {
                blacklist_length: if songs.len() > 10 { songs.len().div(2) } else { songs.len().saturating_sub(3) }
            },
            songs,
            song_indices: Vec::new(),
            song_indices_index: 0,
        }
    }

    fn new_song_index(&self) -> usize {
        match self.mode {
            PlaylistShuffle::Normal => {
                if let Some(last_song_index) = self.song_indices.last().cloned() {
                    (last_song_index + 1) % self.songs.len()
                } else {
                    0
                }
            },
            PlaylistShuffle::Random => {
                self.songs.iter().enumerate().map(|(i, _)| i).choose(&mut rand::thread_rng()).unwrap()
            },
            PlaylistShuffle::SmartRandom { blacklist_length } => {
                let song_indices: Vec<usize> = self.songs.iter().enumerate().map(|(i, _)| i).collect::<Vec<_>>();
                let blacklisted_songs = &self.song_indices[self.song_indices.len().saturating_sub(blacklist_length)..self.song_indices.len()];
                let allowed_song_indices = song_indices.into_iter().filter(|i| blacklisted_songs.iter().all(|b| i != b)).collect::<Vec<_>>();
                allowed_song_indices.into_iter().choose(&mut rand::thread_rng()).unwrap()
            },
        }
    }
}

impl PlaylistSeekable for Playlist {
    fn seek(&mut self, offset: isize) -> Option<Song> {
        self.song_indices_index = self.song_indices_index.checked_add_signed(offset)?;
        if self.song_indices_index >= 0xFFFF { // Failsafe for if somehow we generate alot of song indices.
            panic!("Something went very very wrong. ( ˘︹˘ )");
        }
        while self.song_indices_index >= self.song_indices.len() {
            self.song_indices.push(self.new_song_index());
        }
        self.songs.get(self.song_indices[self.song_indices_index]).cloned()
    }
}


