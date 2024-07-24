
/*
    The reason yt-dlp-music-player isn't using souvlaki anymore for media controls:
        - A lot of unimplemented stuff (Mainly volume control)
        - Annoying to use event receiver
        - Bad error type

    Only windows is currently implemented.
    & Extra stuff like volume controls are N.Y.I.
*/

#![allow(dead_code)]

use std::{ffi::c_void, time::Duration};
use anyhow::{anyhow, Result};

// TODO: Move all MediaControls stuff to a trait, so that it's harder to fuck up



#[derive(Debug, Clone)]
pub enum MediaControlsEvent {
    Play,
    Pause,
    TogglePlayPause,
    Stop,

    Next,
    Previous,

    VolumeToggleMute,
    VolumeMute,
    VolumeUnmute,
    SetVolume(f32),
    VolumeDown,
    VolumeUp,
}

#[derive(Debug, Clone)]
pub enum MediaControlsPlayback {
    Playing(Option<Duration>),
    Paused(Option<Duration>),
    Stopped,
}

#[derive(Debug, Clone, Default)]
pub struct MediaControlsMetadata {
    pub title: Option<String>,
    pub album: Option<String>,
    pub artist: Option<String>,
    /// May either be actual URL or "file://" URL
    pub cover_url: Option<String>,
    pub duration: Option<Duration>,
}



#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use self::windows::MediaControls;



#[derive(Debug)]
pub struct CreateMediaControlsMultiOSOptions {
    pub hwnd: Option<*mut c_void>,
}

/// MediaControls::new() may have different parameters for each platform.
/// So this function unifies creation of MediaControls.
pub fn create_media_controls_multi_os(options: CreateMediaControlsMultiOSOptions) -> Result<MediaControls> {
    #[cfg(target_os = "windows")]
    {
        if let Some(hwnd) = options.hwnd {
            Ok(windows::MediaControls::new(hwnd)?)
        } else {
            Err(anyhow!("createMediaControlsMultiOS options requires hwnd to be set."))
        }
    }
}
