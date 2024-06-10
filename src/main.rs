
use std::{error::Error, fs::{self, File}, io::BufReader, path::PathBuf, process::Command};
use clap::Parser;
use rand::{thread_rng, seq::SliceRandom};
use rodio::Sink;



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

    update_playlist(&playlist_directory, &args.yt_dlp_path, &args.ffmpeg_path, &url::Url::parse(&format!("https://www.youtube.com/playlist?list={}", &playlist_id))?)?;

    loop {
        let binding = fs::read_dir(&playlist_directory)?
            .collect::<Result<Vec<_>, _>>()?;
        let mut song_queue = binding
            .iter()
            .filter(|entry| entry.file_name() != "archive.txt")
            .collect::<Vec<_>>();

        song_queue.shuffle(&mut thread_rng());
        
        for song_file in song_queue {
            println!("Playing: {}", song_file.file_name().to_string_lossy());

            let (_stream, stream_handle) = rodio::OutputStream::try_default()?;
            let sink = Sink::try_new(&stream_handle)?;
            sink.set_volume(0.25);

            let file = File::open(song_file.path())?;
            let source = rodio::Decoder::new(BufReader::new(file))?;
            sink.append(source);
            sink.play();

            sink.sleep_until_end();
        }
    }
}
