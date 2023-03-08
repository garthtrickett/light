use dioxus_desktop::{tao::window::WindowId, DesktopContext};
// use kit::icons::Icon;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, rc::Weak};
use uuid::Uuid;

use super::notifications::Notifications;

#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
pub struct WindowMeta {
    pub focused: bool,
    pub maximized: bool,
    pub minimized: bool,
    pub width: u32,
    pub height: u32,
    pub minimal_view: bool, // We can use this to detect mobile or portrait mode
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct UI {
    pub notifications: Notifications,
    // stores information related to the current call
    #[serde(skip)]
    pub current_call: Option<Call>,
    #[serde(skip)]
    pub current_debug_logger: Option<DebugLogger>,
    // false: the media player is anchored in place
    // true: the media player can move around
    #[serde(skip)]
    pub popout_player: bool,
    #[serde(skip)]
    pub toast_notifications: HashMap<Uuid, ToastNotification>,
    pub theme: Option<Theme>,
    pub enable_overlay: bool,
    pub sidebar_hidden: bool,
    pub metadata: WindowMeta,
}

impl Drop for UI {
    fn drop(&mut self) {}
}

impl UI {
    fn take_popout_id(&mut self) -> Option<WindowId> {
        self.popout_player = false;
        match self.current_call.take() {
            Some(mut call) => {
                let id = call.take_window_id();
                self.current_call = Some(call);
                id
            }
            None => None,
        }
    }

    fn take_debug_logger_id(&mut self) -> Option<WindowId> {
        match self.current_debug_logger.take() {
            Some(mut debug_logger) => {
                let id = debug_logger.take_window_id();
                self.current_debug_logger = None;
                id
            }
            None => None,
        }
    }

    pub fn get_meta(&self) -> WindowMeta {
        self.metadata.clone()
    }

    pub fn is_minimal_view(&self) -> bool {
        self.metadata.minimal_view
    }

    pub fn clear_popout(&mut self, desktop_context: DesktopContext) {
        if let Some(id) = self.take_popout_id() {
            desktop_context.close_window(id);
        };
    }
    pub fn set_popout(&mut self, id: WindowId) {
        self.current_call = Some(Call::new(Some(id)));
        self.popout_player = true;
    }
    pub fn set_debug_logger(&mut self, id: WindowId) {
        self.current_debug_logger = Some(DebugLogger::new(Some(id)));
    }
    pub fn clear_debug_logger(&mut self, desktop_context: DesktopContext) {
        if let Some(id) = self.take_debug_logger_id() {
            desktop_context.close_window(id);
        };
    }

    pub fn toggle_muted(&mut self) {
        self.current_call = self.current_call.clone().map(|mut x| {
            x.muted = !x.muted;
            x
        });
    }

    pub fn toggle_silenced(&mut self) {
        self.current_call = self.current_call.clone().map(|mut x| {
            x.silenced = !x.silenced;
            x
        });
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Call {
    // displays the current  video stream
    // may need changing later to accommodate video streams from multiple participants
    #[serde(skip)]
    pub popout_window_id: Option<WindowId>,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub silenced: bool,
}

impl Call {
    pub fn new(popout_window_id: Option<WindowId>) -> Self {
        Self {
            popout_window_id,
            muted: false,
            silenced: false,
        }
    }

    pub fn take_window_id(&mut self) -> Option<WindowId> {
        self.popout_window_id.take()
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct DebugLogger {
    #[serde(skip)]
    pub debug_logger_window_id: Option<WindowId>,
}

impl DebugLogger {
    pub fn new(popout_window_id: Option<WindowId>) -> Self {
        Self {
            debug_logger_window_id: popout_window_id,
        }
    }

    pub fn take_window_id(&mut self) -> Option<WindowId> {
        self.debug_logger_window_id.take()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct ToastNotification {
    pub title: String,
    pub content: String,
    #[serde(skip)]
    // pub icon: Option<bool>,
    initial_time: u32,
    remaining_time: u32,
}

impl ToastNotification {
    pub fn init(title: String, content: String, icon: Option<bool>, timeout: u32) -> Self {
        Self {
            title,
            content,
            // icon,
            initial_time: timeout,
            remaining_time: timeout,
        }
    }
    pub fn remaining_time(&self) -> u32 {
        self.remaining_time
    }

    pub fn reset_time(&mut self) {
        self.remaining_time = self.initial_time
    }

    pub fn decrement_time(&mut self) {
        if self.remaining_time > 0 {
            self.remaining_time -= 1;
        }
    }
}

#[derive(PartialEq, Clone, Debug, Default, Deserialize, Serialize)]
pub struct Theme {
    pub filename: String,
    pub name: String,
    pub styles: String,
}
