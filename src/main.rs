// TODO: Hide console window, only if ran from explorer.exe
// FIXME: Only auto hide in release mode
#![windows_subsystem = "windows"]

mod config;
mod playlist;
mod song;

use config::Config;
use playlist::Playlist;
use song::Song;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use rodio::{OutputStream, OutputStreamHandle, Sink};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use std::{ffi::c_void, fs, path::PathBuf, process::{Command, Stdio}, sync::mpsc::{self, Receiver}, time::Duration};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};
use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, window::{Window, WindowId}};
use anyhow::{anyhow, Result};



struct App {
    config: Config,
    window: Option<Window>,
    tray_icon: Option<TrayIcon>,
    controls: Option<MediaControls>,
    controls_recv: Option<Receiver<MediaControlEvent>>,
    _stream: (OutputStream, OutputStreamHandle),
    sink: Sink,
    playlist: Playlist,
}

impl App {
    pub fn new(config: Config, playlist: Playlist) -> Result<App> {
        let (stream, handle) = rodio::OutputStream::try_default()?;
        let sink = Sink::try_new(&handle)?;

        sink.set_volume(config.volume as f32);

        Ok(App {
            config,
            window: None,
            tray_icon: None,
            controls: None,
            controls_recv: None,
            _stream: (stream, handle),
            sink,
            playlist,
        })
    }

    fn update_song(&mut self) -> Result<()> {
        let song = self.playlist.seek(0).unwrap();

        println!("Playing: {}", song.name());

        if let Some(controls) = &mut self.controls {
            controls.set_metadata(MediaMetadata {
                title: Some(&song.name()),
                ..MediaMetadata::default()
            }).expect("Failed to set media metadata.");
        }

        Ok(())
    }

    fn seek_song(&mut self, offset: isize) -> Result<()> {
        let was_playing = !self.sink.is_paused();
        self.sink.clear();
        if let Some(song) = self.playlist.seek(offset) {
            song.sink_load(&mut self.sink, self.config.loudness_normalization)?;
            self.update_song()?;
            if was_playing {
                self.sink.play()
            }
        }
        Ok(())
    }

    fn update_playback(&mut self) -> Result<()> {
        if let Some(controls) = &mut self.controls {
            if self.sink.empty() {
                controls.set_playback(MediaPlayback::Stopped).expect("Failed to set playback on media controls");
            } else if self.sink.is_paused() {
                controls.set_playback(MediaPlayback::Paused { progress: None }).expect("Failed to set playback on media controls");
            } else {
                controls.set_playback(MediaPlayback::Playing { progress: None }).expect("Failed to set playback on media controls");
            }
        }
        Ok(())
    }

    fn is_playing(&self) -> bool {
        !self.sink.is_paused() && !self.sink.empty()
    }

    fn play(&mut self) -> Result<()> {
        self.sink.play();
        self.update_playback()?;
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.sink.pause();
        self.update_playback()?;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.sink.stop();
        self.update_playback()?;
        Ok(())
    }

    fn create_window(event_loop: &ActiveEventLoop) -> Result<Window> {
        Ok(event_loop.create_window(
            Window::default_attributes()
                .with_visible(false)
                .with_title("yt-dlp-music-player"),
        )?)
    }

    fn create_tray_icon() -> Result<TrayIcon> {
        // TODO: Icon (Icon as current song image.)
        let icon = tray_icon::Icon::from_rgba(vec![255, 0, 255, 255], 1, 1)?;
        Ok(TrayIconBuilder::new()
            .with_title("yt-dlp-music-player")
            .with_tooltip("yt-dlp-music-player\nLeft: Next\nRight: Previous\nMiddle: Exit")
            .with_id("yt-dlp-music-player")
            .with_icon(icon)
            .build()?)
    }

    fn create_controls(window: &Window) -> Result<MediaControls> {
        #[cfg(not(target_os = "windows"))]
        let hwnd = None;

        #[cfg(target_os = "windows")]
        let hwnd = match window.window_handle()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(h.hwnd.get() as *mut c_void),
            _ => return Err(anyhow!("Failed to get hwnd for window.")),
        };

        let controls = MediaControls::new(PlatformConfig {
            dbus_name: "org.vulae.YtDlpMusicPlayer",
            display_name: "yt-dlp-music-player",
            hwnd,
        }).expect("Failed to get media controls.");

        Ok(controls)
    }

    fn process_media_events(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let mut events = Vec::new();
        if let Some(rx) = &self.controls_recv {
            for event in rx.try_iter() {
                events.push(event);
            }
        }

        for event in events {
            match event {
                MediaControlEvent::Play => self.play()?,
                MediaControlEvent::Pause => self.pause()?,
                MediaControlEvent::Toggle => {
                    if self.is_playing() {
                        self.pause()?
                    } else {
                        self.play()?
                    }
                }
                MediaControlEvent::Next => self.seek_song(1)?,
                MediaControlEvent::Previous => self.seek_song(-1)?,
                MediaControlEvent::Stop => self.stop()?,
                MediaControlEvent::Seek(direction) => println!("Seek: {:#?}", direction),
                MediaControlEvent::SeekBy(direction, duration) => {
                    println!("Seek By: {:#?} {:#?}", direction, duration)
                }
                MediaControlEvent::SetPosition(position) => {
                    println!("Set Position: {:#?}", position)
                }
                MediaControlEvent::SetVolume(volume) => println!("Set Volume: {}", volume),
                MediaControlEvent::OpenUri(uri) => println!("Open URI: {}", uri),
                MediaControlEvent::Raise => println!("Raise"),
                MediaControlEvent::Quit => event_loop.exit(),
            }
        }

        Ok(())
    }

    fn process_tray_icon_events(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click { button, button_state: tray_icon::MouseButtonState::Down, id: _, position: _, rect: _ } = event {
                match button {
                    tray_icon::MouseButton::Left => self.seek_song(1)?,
                    tray_icon::MouseButton::Right => self.seek_song(-1)?,
                    tray_icon::MouseButton::Middle => event_loop.exit(),
                }
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
        controls
            .attach(move |e| tx.send(e).unwrap())
            .expect("Failed to attach to media controls");

        self.window = Some(window);
        self.tray_icon = Some(tray_icon);
        self.controls = Some(controls);
        self.controls_recv = Some(rx);

        self.seek_song(0).unwrap();
        self.play().unwrap();
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

                if !self.sink.is_paused() && self.sink.empty() {
                    self.seek_song(1).unwrap();
                    self.sink.play();
                }

                std::thread::sleep(Duration::from_millis(100));
            }
            winit::event::StartCause::Init => event_loop.set_control_flow(ControlFlow::Poll),
            _ => {}
        }
    }
}

fn update_playlist(playlist_archive_directory: &PathBuf, yt_dlp_path: &PathBuf, ffmpeg_path: &PathBuf, url: &url::Url) -> Result<()> {
    // Update playlist archive directory with yt-dlp
    println!("Updating playlist archive. . .");
    let mut playlist_archive_file = playlist_archive_directory.clone();
    playlist_archive_file.push("archive.txt");

    let mut cmd = Command::new(yt_dlp_path)
        // .arg("-f").arg("bestaudio")
        .arg("--ffmpeg-location").arg(ffmpeg_path)
        .arg("-x")
        .arg("--audio-format").arg("m4a") // TODO: What is the best sounding audio format for tiniest file size?
        .arg("--paths").arg(playlist_archive_directory)
        // Loudness normalization.
        // NOTE: this is disable and instead implemented in song::Song due to being unable to reliably normalize the loudness.
        // .arg("--postprocessor-args").arg("ffmpeg:-af volume=0dB")
        .arg("-o").arg("%(id)s %(title)s.%(ext)s")
        .arg("--download-archive").arg(&playlist_archive_file)
        .arg("--write-thumbnail")
        .arg(url.to_string())
        // .arg("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
        .stdout(Stdio::inherit()) // TODO: Is this fine to do?
        .spawn()?;

    cmd.wait()?;

    println!("Done updating playlist archive.");
    Ok(())
}

fn main() -> Result<()> {
    let config = Config::load()?;

    // Get playlist ID
    let playlist_id: String = match url::Url::parse(&config.yt_playlist) {
        Ok(url) => {
            if let Some(playlist_id) =
                url.query_pairs().find_map(
                    |(name, value)| {
                        if name == "list" {
                            Some(value)
                        } else {
                            None
                        }
                    },
                )
            {
                playlist_id.to_string()
            } else {
                panic!("Invalid URL.");
            }
        }
        Err(_) => config.yt_playlist.clone(),
    };
    println!("Playlist ID: {}", &playlist_id);

    // Get playlist directory
    let mut playlist_directory = std::env::current_dir()?;
    playlist_directory.push(&playlist_id);
    fs::create_dir_all(&playlist_directory)?;
    println!("Playlist archive: {:#?}", &playlist_directory);

    if !config.skip_playlist_update {
        update_playlist(
            &playlist_directory,
            &config.yt_dlp_path,
            &config.ffmpeg_path,
            &url::Url::parse(&format!(
                "https://www.youtube.com/playlist?list={}",
                &playlist_id
            ))?,
        )?;
    }

    let songs = Song::load_playlist_directory(&playlist_directory)?;
    let playlist = Playlist::new(songs);

    let event_loop = EventLoop::new()?;
    let mut app = App::new(config, playlist)?;
    event_loop.run_app(&mut app)?;

    Ok(())
}
