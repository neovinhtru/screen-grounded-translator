#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod api;
mod gui;
#[cfg(target_os = "windows")]
mod overlay;
mod icon_gen;
mod model_config;
mod platform;
#[cfg(target_os = "macos")]
mod overlay_gui;

use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;
use image::ImageBuffer;
use config::{Config, load_config};
use tray_icon::{TrayIconBuilder, menu::{Menu, MenuItem}};
use platform::{setup_platform, spawn_hotkey_listener};

pub struct AppState {
    pub config: Config,
    pub original_screenshot: Option<ImageBuffer<image::Rgba<u8>, Vec<u8>>>,
    pub hotkey_updated: bool,
    pub model_selector: model_config::ModelSelector,
    // macOS Overlay State
    pub show_overlay: bool,
    pub egui_ctx: Option<eframe::egui::Context>,
    pub selection_rect: Option<eframe::egui::Rect>,
    pub translation_result: Option<String>,
    pub monitor_scale_factor: f32,
    pub overlay_origin: (i32, i32),
    pub overlay_geometry: Option<(u32, u32)>,
    pub screens: Vec<ScreenRect>,
    pub copy_feedback_time: Option<std::time::Instant>,
}

#[derive(Clone, Copy, Debug)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub width_logical: i32,
    pub height_logical: i32,
    pub scale_factor: f32,
}

lazy_static! {
    pub static ref APP: Arc<Mutex<AppState>> = Arc::new(Mutex::new(AppState {
        config: load_config(),
        original_screenshot: None,
        hotkey_updated: false,
        model_selector: model_config::ModelSelector::new(model_config::USE_MODEL_ROTATION),
        show_overlay: false,
        egui_ctx: None,
        selection_rect: None,
        translation_result: None,
        monitor_scale_factor: 1.0,
        overlay_origin: (0, 0),
        overlay_geometry: None,
        screens: Vec::new(),
        copy_feedback_time: None,
    }));
}

fn main() -> eframe::Result<()> {
    let _platform_handle = match setup_platform() {
        Ok(Some(h)) => h,
        Ok(None) => return Ok(()), // Exit if single instance check failed (or signalled)
        Err(e) => {
            eprintln!("Platform setup failed: {}", e);
            return Ok(());
        }
    };

    spawn_hotkey_listener();

    let mut viewport_builder = eframe::egui::ViewportBuilder::default()
        .with_inner_size([400.0, 650.0])
        .with_resizable(true);
    
    // Set window icon - embedded in binary
    let app_icon_bytes = include_bytes!("../assets/app-icon-small.png");
    if let Ok(img) = image::load_from_memory(app_icon_bytes) {
        let img_rgba = img.to_rgba8();
        let (width, height) = img_rgba.dimensions();
        let icon_data = eframe::egui::IconData {
            rgba: img_rgba.to_vec(),
            width,
            height,
        };
        viewport_builder = viewport_builder.with_icon(std::sync::Arc::new(icon_data));
    }
    
    let options = eframe::NativeOptions {
        viewport: viewport_builder,
        ..Default::default()
    };
    
    let initial_config = APP.lock().unwrap().config.clone();

    eframe::run_native(
        "Screen Grounded Translator",
        options,
        Box::new(move |cc| {
            gui::configure_fonts(&cc.egui_ctx);
            
            // Store initial context
            {
                let mut app = APP.lock().unwrap();
                app.egui_ctx = Some(cc.egui_ctx.clone());
            }

            // Initialize Tray Icon INSIDE the event loop (required for macOS winit compatibility)
            let tray_menu = Menu::new();
            let settings_i = MenuItem::with_id("1002", "Settings", true, None);
            let quit_i = MenuItem::with_id("1001", "Quit", true, None);
            let _ = tray_menu.append(&settings_i);
            let _ = tray_menu.append(&quit_i);

            let icon = icon_gen::generate_icon();
            let tray_icon = TrayIconBuilder::new()
                .with_menu(Box::new(tray_menu.clone()))
                .with_tooltip("Screen Grounded Translator (nganlinh4)")
                .with_icon(icon)
                .build()
                .unwrap();

            Box::new(gui::SettingsApp::new(initial_config, APP.clone(), tray_icon, tray_menu, cc.egui_ctx.clone()))
        }),
    )
}

