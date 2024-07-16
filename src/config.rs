
use std::{fs, path::PathBuf};
use clap::{ArgGroup, Parser};
use serde::Deserialize;
use anyhow::Result;



#[derive(Parser, Deserialize, Debug)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
#[command(group = ArgGroup::new("all_args").required(false).args(&["yt_dlp_path", "ffmpeg_path"]))]
struct ConfigParser {
    #[arg(index = 1, group = "all_args")]
    yt_dlp_path: Option<PathBuf>,
    #[arg(index = 2, group = "all_args")]
    ffmpeg_path: Option<PathBuf>,
    #[arg(short = 'p', long)]
    yt_playlist: Option<String>,
    #[arg(short, long)]
    skip_playlist_update: Option<bool>,
    #[arg(short, long)]
    volume: Option<f64>,
    #[arg(short, long)]
    loudness_normalization: Option<bool>,
}

impl ConfigParser {
    pub fn merge(a: ConfigParser, b: ConfigParser) -> ConfigParser {
        ConfigParser {
            yt_dlp_path: a.yt_dlp_path.or(b.yt_dlp_path),
            ffmpeg_path: a.ffmpeg_path.or(b.ffmpeg_path),
            yt_playlist: a.yt_playlist.or(b.yt_playlist),
            skip_playlist_update: a.skip_playlist_update.or(b.skip_playlist_update),
            volume: a.volume.or(b.volume),
            loudness_normalization: a.loudness_normalization.or(b.loudness_normalization),
        }
    }
}



#[derive(Debug, Clone)]
pub struct Config {
    pub yt_dlp_path: PathBuf,
    pub ffmpeg_path: PathBuf,
    pub yt_playlist: String,
    pub skip_playlist_update: bool,
    pub volume: f64,
    pub loudness_normalization: bool,
}

impl Config {
    pub fn load() -> Result<Config> {
        let mut partial_config = ConfigParser::parse();

        #[cfg(debug_assertions)]
        if let Ok(str) = fs::read_to_string(&"./target/debug/config.json") {
            partial_config = ConfigParser::merge(partial_config, serde_json::from_str(&str)?);
        }
        #[cfg(not(debug_assertions))]
        if let Ok(str) = fs::read_to_string(&"./config.json") {
            partial_config = ConfigParser::merge(partial_config, serde_json::from_str(&str)?);
        }

        Ok(Config {
            yt_dlp_path: partial_config.yt_dlp_path.expect("CLI or Config must have yt_dlp_path set"),
            ffmpeg_path: partial_config.ffmpeg_path.expect("CLI or Config must have ffmpeg_path set"),
            yt_playlist: partial_config.yt_playlist.expect("CLI or Config must have yt_playlist set"),
            skip_playlist_update: partial_config.skip_playlist_update.unwrap_or(false),
            volume: partial_config.volume.unwrap_or(0.5),
            loudness_normalization: partial_config.loudness_normalization.unwrap_or(true),
        })
    }
}


