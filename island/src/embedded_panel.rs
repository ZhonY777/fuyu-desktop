//! Share the settings painter with WinUI without creating a second window.
use crate::{app::{App, Cmd}, preview_raster::Raster};
use eframe::egui::{self, Event, Key, Modifiers, MouseWheelUnit, PointerButton};
use std::{cell::RefCell, sync::Mutex, time::{Duration, Instant}};

#[derive(Clone, Copy, PartialEq)]
struct Surface {
    width: u32,
    height: u32,
    scale: f32,
    dark: bool,
}

#[derive(Default)]
struct Exchange {
    surface: Option<Surface>,
    events: Vec<Event>,
    modifiers: Modifiers,
    frame: Vec<u8>,
    drawn_surface: Option<Surface>,
    generation: u64,
}

static EXCHANGE: std::sync::LazyLock<Mutex<Exchange>> = std::sync::LazyLock::new(|| Mutex::new(Exchange::default()));

struct Painter {
    context: egui::Context,
    raster: Raster,
    pixels: Vec<u8>,
    start: Instant,
    last: Option<Instant>,
}

thread_local! {
    static PAINTER: RefCell<Option<Painter>> = const { RefCell::new(None) };
}

pub fn active() -> bool {
    EXCHANGE.lock().unwrap().surface.is_some()
}

#[no_mangle]
pub extern "system" fn fuyu_island_settings_surface(width: u32, height: u32, scale: f32, dark: u32) {
    let mut exchange = EXCHANGE.lock().unwrap();
    let surface = (width > 0 && height > 0 && width <= 4096 && height <= 4096
        && u64::from(width) * u64::from(height) <= 5_000_000
        && scale.is_finite() && (0.75..=4.0).contains(&scale))
        .then_some(Surface { width, height, scale, dark: dark != 0 });
    if exchange.surface == surface {
        return;
    }
    exchange.surface = surface;
    if surface.is_none() {
        exchange.events.clear();
        exchange.modifiers = Modifiers::default();
        exchange.frame.clear();
        exchange.drawn_surface = None;
    }
    drop(exchange);
    crate::embedded::repaint();
}

fn key(value: u32) -> Option<Key> {
    Some(match value {
        8 => Key::Backspace, 9 => Key::Tab, 13 => Key::Enter, 27 => Key::Escape,
        33 => Key::PageUp, 34 => Key::PageDown, 35 => Key::End, 36 => Key::Home,
        37 => Key::ArrowLeft, 38 => Key::ArrowUp, 39 => Key::ArrowRight, 40 => Key::ArrowDown,
        46 => Key::Delete, 65 => Key::A, 67 => Key::C, 86 => Key::V, 88 => Key::X,
        89 => Key::Y, 90 => Key::Z,
        _ => return None,
    })
}

#[no_mangle]
pub extern "system" fn fuyu_island_settings_input(kind: u32, x: f32, y: f32, value: u32, flags: u32) {
    let modifiers = Modifiers {
        shift: flags & 1 != 0, ctrl: flags & 2 != 0, alt: flags & 4 != 0,
        command: flags & 2 != 0, mac_cmd: false,
    };
    let position = egui::pos2(x, y);
    let event = match kind {
        0 => Event::PointerMoved(position),
        1 | 2 => Event::PointerButton {
            pos: position,
            button: if value == 1 { PointerButton::Secondary } else { PointerButton::Primary },
            pressed: kind == 1,
            modifiers,
        },
        3 => Event::MouseWheel { unit: MouseWheelUnit::Point, delta: egui::vec2(x, y), modifiers },
        4 if modifiers.ctrl && value == 67 => Event::Copy,
        4 if modifiers.ctrl && value == 88 => Event::Cut,
        4 if modifiers.ctrl && value == 86 => {
            let Ok(text) = clipboard_win::get_clipboard_string() else { return };
            Event::Paste(text)
        }
        4 => {
            let Some(key) = key(value) else { return };
            Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
        }
        5 => Event::PointerGone,
        _ => return,
    };
    enqueue(event, modifiers);
}

fn enqueue(event: Event, modifiers: Modifiers) {
    let mut exchange = EXCHANGE.lock().unwrap();
    if exchange.surface.is_none() {
        return;
    }
    if matches!(event, Event::PointerMoved(_))
        && matches!(exchange.events.last(), Some(Event::PointerMoved(_))) {
        exchange.events.pop();
    }
    if exchange.events.len() < 512 {
        exchange.events.push(event);
    }
    exchange.modifiers = modifiers;
    drop(exchange);
    crate::embedded::repaint();
}

#[no_mangle]
pub unsafe extern "system" fn fuyu_island_settings_text(text: *const u16, length: usize) {
    if text.is_null() || length == 0 || length > 32_768 {
        return;
    }
    // WinUI pins this UTF-16 buffer for the duration of the call.
    let text = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) });
    enqueue(Event::Text(text), Modifiers::default());
}

#[no_mangle]
pub unsafe extern "system" fn fuyu_island_settings_frame(buffer: *mut u8, capacity: usize, generation: u64) -> u64 {
    let exchange = EXCHANGE.lock().unwrap();
    if buffer.is_null() || exchange.generation == generation || exchange.surface.is_none()
        || exchange.surface != exchange.drawn_surface || exchange.frame.is_empty()
        || capacity != exchange.frame.len()
    {
        return 0;
    }
    // The host owns the destination, which is pinned until this copy returns.
    unsafe { std::ptr::copy_nonoverlapping(exchange.frame.as_ptr(), buffer, capacity) };
    exchange.generation
}

pub(crate) fn render(app: &mut App, root: &egui::Context, autostart: bool) {
    let mut exchange = EXCHANGE.lock().unwrap();
    let Some(surface) = exchange.surface else { return };
    let dirty = exchange.drawn_surface != Some(surface) || !exchange.events.is_empty();
    let events = std::mem::take(&mut exchange.events);
    let modifiers = exchange.modifiers;
    drop(exchange);
    root.request_repaint_after(Duration::from_millis(210));

    PAINTER.with_borrow_mut(|painter| {
        let painter = painter.get_or_insert_with(|| {
            let context = egui::Context::default();
            crate::theme::load_fonts(&context);
            egui_extras::install_image_loaders(&context);
            Painter { context, raster: Raster::new(1, 1), pixels: Vec::new(), start: Instant::now(), last: None }
        });
        if !dirty && painter.last.is_some_and(|time| time.elapsed() < Duration::from_millis(200)) {
            return;
        }

        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(
                surface.width as f32 / surface.scale, surface.height as f32 / surface.scale,
            ))),
            time: Some(painter.start.elapsed().as_secs_f64()),
            system_theme: Some(if surface.dark { egui::Theme::Dark } else { egui::Theme::Light }),
            events, modifiers, ..Default::default()
        };
        input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(surface.scale);
        let output = painter.context.run(input, |ctx| app.settings_contents(ctx, autostart, true));
        for command in output.platform_output.commands {
            match command {
                egui::OutputCommand::CopyText(text) => { let _ = clipboard_win::set_clipboard_string(&text); }
                egui::OutputCommand::OpenUrl(url) => app.pending.push(Cmd::Open(url.url.into())),
                _ => {}
            }
        }
        let shapes = painter.context.tessellate(output.shapes, output.pixels_per_point);
        painter.raster.resize(surface.width as usize, surface.height as usize);
        painter.raster.buf.fill([0.0; 4]);
        painter.raster.update_textures(&output.textures_delta);
        painter.raster.draw(&shapes, output.pixels_per_point);
        painter.raster.free_textures(&output.textures_delta);
        painter.raster.to_rgba8(&mut painter.pixels);
        for pixel in painter.pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let mut exchange = EXCHANGE.lock().unwrap();
        if exchange.surface == Some(surface) {
            std::mem::swap(&mut exchange.frame, &mut painter.pixels);
            exchange.drawn_surface = Some(surface);
            exchange.generation = exchange.generation.wrapping_add(1).max(1);
        }
        painter.last = Some(Instant::now());
    });
}
