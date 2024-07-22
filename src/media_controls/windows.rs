
// A lot of code taken from: https://github.com/Sinono3/souvlaki/blob/master/src/platform/windows/mod.rs

use std::{ffi::c_void, sync::{Arc, Mutex}};
use windows::{core::HSTRING, Foundation::{EventRegistrationToken, TimeSpan, TypedEventHandler, Uri}, Media::{MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls, SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs, SystemMediaTransportControlsDisplayUpdater, SystemMediaTransportControlsTimelineProperties}, Storage::Streams::RandomAccessStreamReference, Win32::{Foundation::HWND, System::WinRT::ISystemMediaTransportControlsInterop}};
use anyhow::Result;
use super::{MediaControlsEvent, MediaControlsMetadata, MediaControlsPlayback};

/*
    Seems like volume controls do not work *well* on windows.

    The only way I could see of volume controls working is by
    hijacking key events and if its a media volume control event,
    check if the current selected media item is for this MediaControls
    and if it is, then change the HWND volume.
*/

pub struct MediaControls {
    controls: SystemMediaTransportControls,
    display_updater: SystemMediaTransportControlsDisplayUpdater,
    timeline_properties: SystemMediaTransportControlsTimelineProperties,
    event_queue: Arc<Mutex<Vec<MediaControlsEvent>>>,
    button_handler_token: Option<EventRegistrationToken>,
}

impl MediaControls {
    pub fn new(hwnd: *mut c_void) -> Result<MediaControls> {
        let interop: ISystemMediaTransportControlsInterop = windows::core::factory::<
            SystemMediaTransportControls,
            ISystemMediaTransportControlsInterop,
        >()?;

        let controls: SystemMediaTransportControls = unsafe { interop.GetForWindow(HWND(hwnd)) }?;
        let display_updater = controls.DisplayUpdater()?;
        let timeline_properties = SystemMediaTransportControlsTimelineProperties::new()?;

        let mut controls = MediaControls {
            controls, display_updater, timeline_properties,
            event_queue: Arc::new(Mutex::new(Vec::new())),
            button_handler_token: None,
        };

        controls.init()?;

        Ok(controls)
    }

    fn init(&mut self) -> Result<()> {
        self.controls.SetIsEnabled(true)?;
        self.controls.SetIsPlayEnabled(true)?;
        self.controls.SetIsPauseEnabled(true)?;
        self.controls.SetIsStopEnabled(true)?;
        self.controls.SetIsNextEnabled(true)?;
        self.controls.SetIsPreviousEnabled(true)?;

        self.display_updater.SetType(MediaPlaybackType::Music)?;

        let event_queue = Arc::clone(&self.event_queue);

        let button_handler = TypedEventHandler::new(
            move |_, args: &Option<_>| {
                let args: &SystemMediaTransportControlsButtonPressedEventArgs = args.as_ref().unwrap();
                let button = args.Button()?;

                let event: Option<MediaControlsEvent> = match button {
                    SystemMediaTransportControlsButton::Play => Some(MediaControlsEvent::Play),
                    SystemMediaTransportControlsButton::Pause => Some(MediaControlsEvent::Pause),
                    SystemMediaTransportControlsButton::Stop => Some(MediaControlsEvent::Stop),
                    SystemMediaTransportControlsButton::Next => Some(MediaControlsEvent::Next),
                    SystemMediaTransportControlsButton::Previous => Some(MediaControlsEvent::Previous),
                    _ => None,
                };

                if let Some(event) = event {
                    let mut event_queue = event_queue.lock().unwrap();
                    event_queue.push(event);
                }

                Ok(())
            }
        );

        self.button_handler_token = Some(self.controls.ButtonPressed(&button_handler)?);

        Ok(())
    }

    fn destroy(&mut self) -> Result<()> {
        if let Some(button_handler_token) = self.button_handler_token {
            self.controls.RemoveButtonPressed(button_handler_token)?;
            self.button_handler_token = None;
        }
        Ok(())
    }



    pub fn next_event(&mut self) -> Option<MediaControlsEvent> {
        let mut event_queue = self.event_queue.lock().unwrap();
        if event_queue.is_empty() {
            None
        } else {
            Some(event_queue.remove(0))
        }
    }

    pub fn set_playback(&mut self, playback: MediaControlsPlayback) -> Result<()> {
        self.controls.SetPlaybackStatus(match playback {
            MediaControlsPlayback::Playing(_) => MediaPlaybackStatus::Playing,
            MediaControlsPlayback::Paused(_) => MediaPlaybackStatus::Paused,
            MediaControlsPlayback::Stopped => MediaPlaybackStatus::Stopped,
        })?;
        self.timeline_properties.SetPosition((match playback {
            MediaControlsPlayback::Playing(Some(progress)) => Some(TimeSpan::from(progress)),
            MediaControlsPlayback::Paused(Some(progress)) => Some(TimeSpan::from(progress)),
            _ => None,
        }).unwrap_or(TimeSpan::default()))?;
        self.controls.UpdateTimelineProperties(&self.timeline_properties)?;
        Ok(())
    }

    pub fn set_metadata(&mut self, metadata: MediaControlsMetadata) -> Result<()> {
        let properties = self.display_updater.MusicProperties()?;

        if let Some(title) = metadata.title {
            properties.SetTitle(&HSTRING::from(title))?;
        }
        if let Some(artist) = metadata.artist {
            properties.SetArtist(&HSTRING::from(artist))?;
        }
        if let Some(album) = metadata.album {
            properties.SetAlbumTitle(&HSTRING::from(album))?;
        }
        if let Some(url) = metadata.cover_url {
            let stream = if url.starts_with("file://") {
                let path = url.trim_start_matches("file://");
                let loader = windows::Storage::StorageFile::GetFileFromPathAsync(&HSTRING::from(path))?;
                let results = loader.get()?;
                loader.Close()?;
                RandomAccessStreamReference::CreateFromFile(&results)?
            } else {
                RandomAccessStreamReference::CreateFromUri(&Uri::CreateUri(&HSTRING::from(url))?)?
            };
            self.display_updater.SetThumbnail(&stream)?;
        }
        if let Some(duration) = metadata.duration {
            self.timeline_properties.SetStartTime(TimeSpan::default())?;
            self.timeline_properties.SetMinSeekTime(TimeSpan::default())?;
            self.timeline_properties.SetEndTime(TimeSpan::from(duration))?;
            self.timeline_properties.SetMaxSeekTime(TimeSpan::from(duration))?;
            self.controls.UpdateTimelineProperties(&self.timeline_properties)?;
        }

        self.display_updater.Update()?;

        Ok(())
    }
}

impl Drop for MediaControls {
    fn drop(&mut self) {
        self.destroy().expect("Failed to destroy MediaControls");
    }
}


