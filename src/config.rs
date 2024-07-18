
// TODO: Refactor all of this, it's pretty ugly, I hope you don't look. . . ◑﹏◐

use std::{fs, path::PathBuf};
use clap::{ArgGroup, Parser};
use serde::Deserialize;
use anyhow::Result;



// I cannot get serde_flat_path to work, so we have to deal with multiple structs for now. . .
#[derive(Deserialize, Debug)]
struct TomlConfigParserProgramPaths {
    #[serde(rename = "yt-dlp-path")]
    yt_dlp_path: Option<PathBuf>,
    #[serde(rename = "ffmpeg-path")]
    ffmpeg_path: Option<PathBuf>,
}

#[derive(Deserialize, Debug)]
struct TomlConfigParserConfig {
    #[serde(rename="yt-playlist")]
    yt_playlist: Option<String>,
    #[serde(rename="skip_playlist_update")]
    skip_playlist_update: Option<bool>,
    volume: Option<f64>,
    #[serde(rename="loudness-normalization")]
    loudness_normalization: Option<bool>,
}

#[derive(Deserialize, Debug)]
struct TomlConfigParser {
    #[serde(rename="program-paths")]
    program_paths: Option<TomlConfigParserProgramPaths>,
    config: Option<TomlConfigParserConfig>,
}



#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
#[command(group = ArgGroup::new("all_args").required(false).args(&["yt_dlp_path", "ffmpeg_path"]))]
struct CliConfigParser {
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



#[derive(Debug)]
struct PartialConfig {
    yt_dlp_path: Option<PathBuf>,
    ffmpeg_path: Option<PathBuf>,
    yt_playlist: Option<String>,
    skip_playlist_update: Option<bool>,
    volume: Option<f64>,
    loudness_normalization: Option<bool>,
}

impl PartialConfig {
    pub fn empty() -> PartialConfig {
        PartialConfig {
            yt_dlp_path: None,
            ffmpeg_path: None,
            yt_playlist: None,
            skip_playlist_update: None,
            volume: None,
            loudness_normalization: None,
        }
    }

    pub fn merge(a: PartialConfig, b: PartialConfig) -> PartialConfig {
        PartialConfig {
            yt_dlp_path: a.yt_dlp_path.or(b.yt_dlp_path),
            ffmpeg_path: a.ffmpeg_path.or(b.ffmpeg_path),
            yt_playlist: a.yt_playlist.or(b.yt_playlist),
            skip_playlist_update: a.skip_playlist_update.or(b.skip_playlist_update),
            volume: a.volume.or(b.volume),
            loudness_normalization: a.loudness_normalization.or(b.loudness_normalization),
        }
    }

    pub fn from_config_file(config_file: &PathBuf) -> Result<PartialConfig> {
        let config: TomlConfigParser = toml::from_str(&fs::read_to_string(&config_file)?)?;
        Ok(PartialConfig {
            yt_dlp_path: config.program_paths.as_ref().and_then(|c| c.yt_dlp_path.clone()),
            ffmpeg_path: config.program_paths.as_ref().and_then(|c| c.ffmpeg_path.clone()),
            yt_playlist: config.config.as_ref().and_then(|c| c.yt_playlist.clone()),
            skip_playlist_update: config.config.as_ref().and_then(|c| c.skip_playlist_update.clone()),
            volume: config.config.as_ref().and_then(|c| c.volume.clone()),
            loudness_normalization: config.config.as_ref().and_then(|c| c.loudness_normalization.clone()),
        })
    }

    pub fn from_cli_args() -> PartialConfig {
        let config: CliConfigParser = CliConfigParser::parse();
        PartialConfig {
            yt_dlp_path: config.yt_dlp_path,
            ffmpeg_path: config.ffmpeg_path,
            yt_playlist: config.yt_playlist,
            skip_playlist_update: config.skip_playlist_update,
            volume: config.volume,
            loudness_normalization: config.loudness_normalization,
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
        let config: PartialConfig = PartialConfig::empty();

        #[cfg(debug_assertions)]
        let config_file = PathBuf::from("./target/debug/config.toml");
        #[cfg(not(debug_assertions))]
        let config_file = PathBuf::from("./config.toml");
        let config = PartialConfig::merge(PartialConfig::from_config_file(&config_file)?, config);

        let config = PartialConfig::merge(PartialConfig::from_cli_args(), config);

        Ok(Config {
            yt_dlp_path: config.yt_dlp_path.expect("CLI or Config must have yt_dlp_path set"),
            ffmpeg_path: config.ffmpeg_path.expect("CLI or Config must have ffmpeg_path set"),
            yt_playlist: config.yt_playlist.expect("CLI or Config must have yt_playlist set"),
            skip_playlist_update: config.skip_playlist_update.unwrap_or(false),
            volume: config.volume.unwrap_or(0.5),
            loudness_normalization: config.loudness_normalization.unwrap_or(true),
        })
    }
}


