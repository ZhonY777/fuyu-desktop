//! Native entry points for running the island inside the desktop host.
use crate::app::App;
use eframe::egui::{Context, ViewportCommand};
use std::sync::{Mutex, OnceLock};

type NotifyHost = extern "system" fn(u32);

const READY: u32 = 1;
const SHOW_DESKTOP: u32 = 2;
const SHOW_DESKTOP_SETTINGS: u32 = 3;
const SHOW_ISLAND_SETTINGS: u32 = 4;

const OPEN_CALENDAR: u32 = 2;
const STOP: u32 = 3;
const OPEN_CATALOG: u32 = 4;

static HOST: OnceLock<NotifyHost> = OnceLock::new();
static STATE: Mutex<HostState> = Mutex::new(HostState {
    context: None,
    commands: Vec::new(),
    stopping: false,
});

struct HostState {
    context: Option<Context>,
    commands: Vec<u32>,
    stopping: bool,
}

#[no_mangle]
pub extern "system" fn fuyu_island_run(notify: Option<NotifyHost>) -> u32 {
    let Some(notify) = notify else { return 2 };
    if HOST.set(notify).is_err() {
        return 2;
    }
    let path = crate::model::config_dir().join("diagnostics").join("host-panic.txt");
    std::panic::set_hook(Box::new(move |info| {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, format!("{info}\n{}", std::backtrace::Backtrace::force_capture()));
    }));
    let result = crate::shell::run_ui(true);
    STATE.lock().unwrap().context = None;
    match result {
        Ok(()) => 0,
        Err(error) => {
            let path = crate::model::config_dir().join("host-startup-error.txt");
            let _ = std::fs::write(path, error.to_string());
            1
        }
    }
}

#[no_mangle]
pub extern "system" fn fuyu_island_command(command: u32) -> u32 {
    if !matches!(command, OPEN_CALENDAR | STOP | OPEN_CATALOG) || HOST.get().is_none() {
        return 1;
    }
    let mut state = STATE.lock().unwrap();
    if state.stopping {
        return 1;
    }
    if command == STOP {
        state.stopping = true;
        state.commands.clear();
    } else if !state.commands.contains(&command) {
        state.commands.push(command);
    }
    let context = state.context.clone();
    drop(state);
    if let Some(context) = context {
        context.request_repaint();
    }
    0
}

pub(crate) fn repaint() {
    let context = STATE.lock().unwrap().context.clone();
    if let Some(context) = context {
        context.request_repaint();
    }
}

pub(crate) fn update(app: &mut App, context: &Context) -> bool {
    let mut state = STATE.lock().unwrap();
    let ready = state.context.is_none() && !state.stopping;
    state.context = Some(context.clone());
    let commands = std::mem::take(&mut state.commands);
    let stopping = state.stopping;
    drop(state);
    if stopping {
        context.send_viewport_cmd(ViewportCommand::Close);
        return true;
    }
    if ready {
        HOST.get().unwrap()(READY);
    }
    app.settings_open = crate::embedded_panel::active();
    for command in commands {
        match command {
            OPEN_CALENDAR => app.open_calendar(None),
            OPEN_CATALOG => app.desktop.request_open(),
            _ => {}
        }
    }
    false
}

pub(crate) fn request_settings() -> bool {
    let Some(notify) = HOST.get() else { return false };
    notify(SHOW_ISLAND_SETTINGS);
    true
}

pub(crate) fn request_desktop(settings: bool) -> bool {
    let Some(notify) = HOST.get() else { return false };
    notify(if settings { SHOW_DESKTOP_SETTINGS } else { SHOW_DESKTOP });
    true
}

pub(crate) fn is_hosted() -> bool {
    HOST.get().is_some()
}
