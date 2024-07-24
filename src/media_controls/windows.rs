
// A lot of code taken from: https://github.com/Sinono3/souvlaki/blob/master/src/platform/windows/mod.rs

use std::{ffi::c_void, sync::{Arc, Mutex}, thread};
use windows::{core::HSTRING, Foundation::{EventRegistrationToken, TimeSpan, TypedEventHandler, Uri}, Media::{Control::GlobalSystemMediaTransportControlsSessionManager, MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls, SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs, SystemMediaTransportControlsDisplayUpdater, SystemMediaTransportControlsTimelineProperties}, Storage::Streams::RandomAccessStreamReference, Win32::{Foundation::{HWND, LPARAM, LRESULT, WPARAM}, System::{LibraryLoader::GetModuleHandleW, WinRT::ISystemMediaTransportControlsInterop}, UI::WindowsAndMessaging::{CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN}}};
use anyhow::Result;
use super::{MediaControlsEvent, MediaControlsMetadata, MediaControlsPlayback};

/*
    Media volume controls *REALLY* suck on Windows.
    This is the only way I could figure out on how to implement them.
    By listening for volume media events, and then hijacking the events if the current media item is yt-dlp-music-player.
*/

// TODO: Refactor, Allow for multiple listeners.
// This also doesn't property clean up once media controls are dropped.
// I just sorta made this fast because I actually have 0 clue on how to properly implement this.
// Also this is very jank, when toggling mute if theres other media playing, it may just break.

static mut EVENT_QUEUES: Vec<Arc<Mutex<Vec<MediaControlsEvent>>>> = vec![];

unsafe extern "system" fn hook_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    
    let mut media_event: Option<MediaControlsEvent> = None;

    if n_code == (HC_ACTION as i32) {
        let kb_struct: &KBDLLHOOKSTRUCT = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
        match w_param.0 as u32 {
            WM_KEYDOWN => {
                match kb_struct.vkCode {
                    0xAD => {
                        media_event = Some(MediaControlsEvent::VolumeToggleMute);
                    },
                    0xAE => {
                        media_event = Some(MediaControlsEvent::VolumeDown);
                    },
                    0xAF => {
                        media_event = Some(MediaControlsEvent::VolumeUp);
                    },
                    _ => { },
                }
            }
            _ => (),
        }
    }

    if let Some(media_event) = media_event {
        // Check if current media item is yt-dlp-music-player.
        match MediaControls::is_active() {
            Ok(true) => {
                // Add to queues
                EVENT_QUEUES.iter().for_each(|event_queue| {
                    let mut event_queue = event_queue.lock().unwrap();
                    event_queue.push(media_event.clone());
                });

                // Cancel keypress
                return LRESULT(1);
            },
            _ => {},
        }
    }

    CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
}

static mut HOOK_PROC_INITIALIZED: bool = false;
unsafe fn init_hook_proc(event_queue: Arc<Mutex<Vec<MediaControlsEvent>>>) -> Result<()> {
    EVENT_QUEUES.push(event_queue);

    if HOOK_PROC_INITIALIZED { return Ok(()) }
    HOOK_PROC_INITIALIZED = true;

    thread::spawn(|| {
        let h_instance = GetModuleHandleW(None).unwrap();
        let hook_id = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), h_instance, 0).unwrap();
        if hook_id.is_invalid() {
            panic!("Invalid hook!");
        }
    
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        UnhookWindowsHookEx(hook_id).unwrap();
    });

    Ok(())
}



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
        // Volume controls just do not seem to work.
        // self.controls.SetIsChannelDownEnabled(true)?;
        // self.controls.SetIsChannelUpEnabled(true)?;

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
                    // SystemMediaTransportControlsButton::ChannelDown => Some(MediaControlsEvent::VolumeDown),
                    // SystemMediaTransportControlsButton::ChannelUp => Some(MediaControlsEvent::VolumeUp),
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

        unsafe { init_hook_proc(Arc::clone(&self.event_queue))? };

        Ok(())
    }

    fn destroy(&mut self) -> Result<()> {
        if let Some(button_handler_token) = self.button_handler_token {
            self.controls.RemoveButtonPressed(button_handler_token)?;
            self.button_handler_token = None;
        }
        Ok(())
    }

    fn is_active() -> Result<bool> {
        // TODO: Better way to test if current active session is this one.
        let media_manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.get()?;
        match media_manager.GetCurrentSession() {
            Ok(media_session) => {
                let media_properties = media_session.TryGetMediaPropertiesAsync()?.get()?;
                match media_properties.AlbumTitle() {
                    Ok(media_album_title) => Ok(media_album_title == "yt-dlp-music-player"),
                    _ => Ok(false),
                }
            },
            _ => Ok(false),
        }
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

        // TEMP: For testing volume controls.
        properties.SetAlbumTitle(&HSTRING::from("yt-dlp-music-player"))?;

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


