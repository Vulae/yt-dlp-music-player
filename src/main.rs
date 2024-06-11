
use std::{error::Error, ffi::c_void, fs::{self, File}, io::BufReader, path::PathBuf, process::Command, sync::mpsc::{self, Receiver}, time::Duration};
use clap::Parser;
use rand::{thread_rng, seq::SliceRandom};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, window::{Window, WindowId}};



#[derive(Default)]
struct App {
    playlist_directory: PathBuf,
    window: Option<Window>,
    controls: Option<MediaControls>,
    controls_recv: Option<Receiver<MediaControlEvent>>,
    stream: Option<(OutputStream, OutputStreamHandle)>,
    sink: Option<Sink>,
}

impl App {
    fn next_song(&mut self) -> Result<(), Box<dyn Error>> {
        let binding = fs::read_dir(&self.playlist_directory)?
            .collect::<Result<Vec<_>, _>>()?;
        let mut song_queue = binding
            .iter()
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".m4a"))
            .collect::<Vec<_>>();

        song_queue.shuffle(&mut thread_rng());

        let song_file = song_queue[0];
        let mut duration = None;

        // Cover can either be WEBP or JPEG.
        // why :(
        let mut cover_file = song_file.file_name().to_string_lossy().trim_end_matches(".m4a").to_string();
        cover_file += ".webp";
        if fs::metadata(&cover_file).is_err() {
            cover_file = cover_file.trim_end_matches(".webp").to_string();
            cover_file += ".jpg";
        }
        let mut cover_file_path = self.playlist_directory.clone();
        cover_file_path.push(cover_file);

        if let Some(sink) = &self.sink {
            println!("Playing: {}", song_file.file_name().to_string_lossy());
    
            let file = File::open(song_file.path())?;
            let source = rodio::Decoder::new(BufReader::new(file))?;
            
            duration = source.total_duration();

            sink.append(source);
            sink.play();
        }

        if self.sink.is_some() {
            self.update_playback(souvlaki::MediaPlayback::Playing { progress: None })?;
            self.update_metadata(MediaMetadata {
                title: Some(&song_file.file_name().to_string_lossy()),
                duration,
                cover_url: Some(&cover_file_path.clone().to_string_lossy()), // FIXME: Why doesn't this display?
                ..MediaMetadata::default()
            })?;
        }

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
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop.create_window(
            Window::default_attributes()
                .with_visible(false)
        ).unwrap();

        let (tx, rx) = mpsc::sync_channel(32);

        let hwnd = match window.window_handle().expect("Failed to get window handle").as_raw() {
            RawWindowHandle::Win32(h) =>  h.hwnd.get() as *mut c_void,
            _ => unreachable!(),
        };
        let mut controls = MediaControls::new(PlatformConfig {
            dbus_name: "yt-dlp-music-player",
            display_name: "yt-dlp-music-player",
            hwnd: Some(hwnd)
        }).expect("Failed to create media controls");
        controls.attach(move |e| tx.send(e).unwrap()).expect("Failed to attach to media controls");
        
        let (stream, stream_handle) = rodio::OutputStream::try_default().expect("Failed to create output stream");
        let sink = Sink::try_new(&stream_handle).expect("Failed to create sink");
        sink.set_volume(0.25);

        self.window = Some(window);
        self.controls = Some(controls);
        self.controls_recv = Some(rx);
        self.stream = Some((stream, stream_handle));
        self.sink = Some(sink);

        self.next_song().expect("Failed to start next song");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => (),
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        let mut events = Vec::new();
        if let Some(rx) = &self.controls_recv {
            for event in rx.try_iter() {
                events.push(event);
            }
        }
        
        for event in events {
            match event {
                MediaControlEvent::Play => {
                    if let Some(sink) = &self.sink {
                        sink.play();
                        self.update_playback(MediaPlayback::Playing { progress: None }).unwrap();
                    }
                },
                MediaControlEvent::Pause => {
                    if let Some(sink) = &self.sink {
                        sink.pause();
                        self.update_playback(MediaPlayback::Paused { progress: None }).unwrap();
                    }
                },
                MediaControlEvent::Toggle => {
                    if let Some(sink) = &self.sink {
                        if sink.is_paused() {
                            sink.play();
                            self.update_playback(MediaPlayback::Playing { progress: None }).unwrap();
                        } else {
                            sink.pause();
                            self.update_playback(MediaPlayback::Paused { progress: None }).unwrap();
                        }
                    }
                },
                MediaControlEvent::Next => {
                    if let Some(sink) = &self.sink {
                        sink.clear();
                    }
                    self.next_song().expect("Failed to start next song");
                },
                MediaControlEvent::Previous => println!("Previous Song"),
                MediaControlEvent::Stop => {
                    if let Some(sink) = &self.sink {
                        sink.clear();
                        self.update_playback(MediaPlayback::Stopped).unwrap();
                    }
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

        match cause {
            winit::event::StartCause::Poll => std::thread::sleep(Duration::from_millis(100)),
            winit::event::StartCause::Init => event_loop.set_control_flow(ControlFlow::Poll),
            _ => { },
        }
    }
}





#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[arg(index = 1)]
    yt_dlp_path: PathBuf,
    #[arg(index = 2)]
    ffmpeg_path: PathBuf,
    #[arg(index = 3)]
    yt_playlist: String,
    #[arg(short, long, default_value_t = false)]
    skip_update: bool,
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
    let args = Cli::parse();

    // Get playlist ID
    let playlist_id: String = match url::Url::parse(&args.yt_playlist) {
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
        Err(_) => args.yt_playlist.clone()
    };
    println!("Playlist ID: {}", &playlist_id);

    // Get playlist directory
    let mut playlist_directory = std::env::current_dir()?;
    playlist_directory.push(&playlist_id);
    fs::create_dir_all(&playlist_directory)?;
    println!("Playlist archive: {:#?}", &playlist_directory);

    if !args.skip_update {
        update_playlist(&playlist_directory, &args.yt_dlp_path, &args.ffmpeg_path, &url::Url::parse(&format!("https://www.youtube.com/playlist?list={}", &playlist_id))?)?;
    }

    let event_loop = EventLoop::new()?;
    let mut app = App::default();
    app.playlist_directory = playlist_directory;
    event_loop.run_app(&mut app)?;

    Ok(())
}