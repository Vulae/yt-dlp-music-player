
// TODO: Hide console window, only if ran from explorer.exe
// FIXME: Only auto hide in release mode
#![windows_subsystem = "windows"]

mod config;
mod player;

use std::{error::Error, ffi::c_void, fs, path::PathBuf, process::Command, sync::mpsc::{self, Receiver}, time::Duration};
use config::Config;
use player::{Player, Song};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, window::{Window, WindowId}};
use rand::{thread_rng, seq::SliceRandom};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};



struct App {
    window: Option<Window>,
    tray_icon: Option<TrayIcon>,
    controls: Option<MediaControls>,
    controls_recv: Option<Receiver<MediaControlEvent>>,
    player: Player,
}

impl App {
    pub fn new(player: Player) -> App {
        App {
            window: None,
            tray_icon: None,
            controls: None,
            controls_recv: None,
            player,
        }
    }

    fn next_song(&mut self) -> Result<(), Box<dyn Error>> {

        self.player.next_song();
        let song = self.player.current_song();

        println!("Playing: {}", song.name());

        self.update_metadata(MediaMetadata {
            title: Some(&song.name()),
            ..MediaMetadata::default()
        })?;

        self.update_playback(souvlaki::MediaPlayback::Playing { progress: None })?;
        self.player.play();

        Ok(())
    }

    fn update_playback(&mut self, playback: MediaPlayback) -> Result<(), Box<dyn Error>> {
        if let Some(controls) = &mut self.controls {
            controls.set_playback(playback).expect("Failed to set playback on media controls");
        }
        Ok(())
    }

    fn update_metadata(&mut self, metadata: MediaMetadata) -> Result<(), Box<dyn Error>> {
        if let Some(controls) = &mut self.controls {
            controls.set_metadata(metadata).expect("Failed to set metadata on media controls"); 
        }
        Ok(())
    }



    fn create_window(event_loop: &ActiveEventLoop) -> Result<Window, Box<dyn Error>> {
        Ok(event_loop.create_window(
            Window::default_attributes()
                .with_visible(false)
                .with_title("yt-dlp-music-player")
        )?)
    }

    fn create_tray_icon() -> Result<TrayIcon, Box<dyn Error>> {
        // TODO: Icon (Icon as current song image.)
        let icon = tray_icon::Icon::from_rgba(vec![255, 0, 255, 255], 1, 1)?;
        Ok(TrayIconBuilder::new()
            .with_title("yt-dlp-music-player")
            .with_tooltip("yt-dlp-music-player\nLeft: Next\nRight: Previous\nMiddle: Exit")
            .with_id("yt-dlp-music-player")
            .with_icon(icon)
            .build()?)
    }

    fn create_controls(window: &Window) -> Result<MediaControls, Box<dyn Error>> {
        #[cfg(not(target_os = "windows"))]
        let hwnd = None;

        #[cfg(target_os = "windows")]
        let hwnd = match window.window_handle()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(h.hwnd.get() as *mut c_void),
            _ => panic!("Could not get HWND"),
        };

        let controls = MediaControls::new(PlatformConfig {
            dbus_name: "org.vulae.YtDlpMusicPlayer",
            display_name: "yt-dlp-music-player",
            hwnd: hwnd
        }).expect("Failed to create media controls");

        Ok(controls)
    }



    fn process_media_events(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
        let mut events = Vec::new();
        if let Some(rx) = &self.controls_recv {
            for event in rx.try_iter() {
                events.push(event);
            }
        }

        for event in events {
            match event {
                MediaControlEvent::Play => {
                    self.player.play();
                    self.update_playback(MediaPlayback::Playing { progress: None })?;
                },
                MediaControlEvent::Pause => {
                    self.player.pause();
                    self.update_playback(MediaPlayback::Paused { progress: None })?;
                },
                MediaControlEvent::Toggle => {
                    if !self.player.is_playing() {
                        self.player.play();
                        self.update_playback(MediaPlayback::Playing { progress: None })?;
                    } else {
                        self.player.pause();
                        self.update_playback(MediaPlayback::Paused { progress: None })?;
                    }
                },
                MediaControlEvent::Next => {
                    self.next_song()?;
                },
                MediaControlEvent::Previous => println!("Previous Song"),
                MediaControlEvent::Stop => {
                    self.player.stop();
                    self.update_playback(MediaPlayback::Stopped)?;
                },
                MediaControlEvent::Seek(direction) => println!("Seek: {:#?}", direction),
                MediaControlEvent::SeekBy(direction, duration) => println!("Seek By: {:#?} {:#?}", direction, duration),
                MediaControlEvent::SetPosition(position) => println!("Set Position: {:#?}", position),
                MediaControlEvent::SetVolume(volume) => println!("Set Volume: {}", volume),
                MediaControlEvent::OpenUri(uri) => println!("Open URI: {}", uri),
                MediaControlEvent::Raise => println!("Raise"),
                MediaControlEvent::Quit => event_loop.exit(),
            }
        }

        Ok(())
    }

    fn process_tray_icon_events(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            match event {
                TrayIconEvent::Click {
                    id: _,
                    position: _,
                    rect: _,
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Down
                } => self.next_song()?,
                TrayIconEvent::Click {
                    id: _,
                    position: _,
                    rect: _,
                    button: tray_icon::MouseButton::Right,
                    button_state: tray_icon::MouseButtonState::Down
                } => println!("Previous Song"),
                TrayIconEvent::Click {
                    id: _,
                    position: _,
                    rect: _,
                    button: tray_icon::MouseButton::Middle,
                    button_state: tray_icon::MouseButtonState::Down
                } => event_loop.exit(),
                _ => { },
            }
        }
        Ok(())
    }

}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = App::create_window(event_loop).unwrap();
        let tray_icon = App::create_tray_icon().unwrap();
        let mut controls = App::create_controls(&window).unwrap();
        
        let (tx, rx) = mpsc::sync_channel(32);
        controls.attach(move |e| tx.send(e).unwrap()).expect("Failed to attach to media controls");

        self.window = Some(window);
        self.tray_icon = Some(tray_icon);
        self.controls = Some(controls);
        self.controls_recv = Some(rx);

        self.next_song().expect("Failed to start next song");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => (),
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        match cause {
            winit::event::StartCause::Poll => {
                self.process_media_events(&event_loop).unwrap();
                self.process_tray_icon_events(&event_loop).unwrap();

                if self.player.is_finished() {
                    self.next_song().expect("Failed to play next song");
                }

                std::thread::sleep(Duration::from_millis(100));
            },
            winit::event::StartCause::Init => event_loop.set_control_flow(ControlFlow::Poll),
            _ => { },
        }
    }
}



fn update_playlist(playlist_archive_directory: &PathBuf, yt_dlp_path: &PathBuf, ffmpeg_path: &PathBuf, url: &url::Url) -> Result<(), Box<dyn Error>> {
    // Update playlist archive directory with yt-dlp
    println!("Updating playlist archive. . .");
    let mut playlist_archive_file = playlist_archive_directory.clone();
    playlist_archive_file.push("archive.txt");
    Command::new(yt_dlp_path)
        // .arg("-f").arg("bestaudio")
        .arg("--ffmpeg-location").arg(ffmpeg_path)
        .arg("-x").arg("--audio-format").arg("m4a")
        .arg("--paths").arg(playlist_archive_directory)
        .arg("--download-archive").arg(&playlist_archive_file)
        .arg("--write-thumbnail")
        .arg(url.to_string())
        // .arg("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
        .output()?;
    println!("Done updating playlist archive.");
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::load()?;

    // Get playlist ID
    let playlist_id: String = match url::Url::parse(&config.yt_playlist) {
        Ok(url) => if let Some(playlist_id) = url.query_pairs().find_map(|(name, value)| {
            if name == "list" {
                Some(value)
            } else {
                None
            }
        }) {
            playlist_id.to_string()
        } else {
            panic!("Invalid URL.");
        },
        Err(_) => config.yt_playlist.clone()
    };
    println!("Playlist ID: {}", &playlist_id);

    // Get playlist directory
    let mut playlist_directory = std::env::current_dir()?;
    playlist_directory.push(&playlist_id);
    fs::create_dir_all(&playlist_directory)?;
    println!("Playlist archive: {:#?}", &playlist_directory);

    if !config.skip_playlist_update {
        update_playlist(&playlist_directory, &config.yt_dlp_path, &config.ffmpeg_path, &url::Url::parse(&format!("https://www.youtube.com/playlist?list={}", &playlist_id))?)?;
    }

    let mut songs = Song::load_playlist_directory(&playlist_directory)?;
    songs.shuffle(&mut thread_rng()); // TODO: Implement actual song shuffling in Player.
    let mut player = Player::new(songs)?;

    player.set_volume(0.25);

    let event_loop = EventLoop::new()?;
    let mut app = App::new(player);
    event_loop.run_app(&mut app)?;

    Ok(())
}