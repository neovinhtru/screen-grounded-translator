#![allow(unexpected_cfgs)]
use cocoa::appkit::{NSApp, NSScreen, NSColor};
use cocoa::base::{nil, NO, YES};
use cocoa::foundation::{NSArray, NSAutoreleasePool, NSString};
use objc::{msg_send, sel, sel_impl, class};

use crate::APP;
use anyhow::Result;
use image::ImageBuffer;
use std::fs::File;
use fs2::FileExt;
use std::path::PathBuf;
use global_hotkey::{GlobalHotKeyManager, hotkey::{HotKey, Code}, GlobalHotKeyEvent};
use screenshots::Screen;
use std::ffi::CStr;

pub struct PlatformHandle {
    _lock_file: File,
}

pub fn setup_platform() -> Result<Option<PlatformHandle>> {
    let lock_path = PathBuf::from("/tmp/screen-grounded-translator.lock");
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&lock_path)?;
    
    if file.try_lock_exclusive().is_err() {
        return Ok(None);
    }
    
    Ok(Some(PlatformHandle { _lock_file: file }))
}

pub fn spawn_hotkey_listener() {
    std::thread::spawn(|| {
        let manager = GlobalHotKeyManager::new().unwrap();
        // Default to Tilde (Backquote)
        let hotkey = HotKey::new(None, Code::Backquote);
        
        if let Err(e) = manager.register(hotkey) {
            println!("Failed to register hotkey: {:?}", e);
        }

        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            if let Ok(event) = receiver.try_recv() {
                if event.state == global_hotkey::HotKeyState::Released {
                    let mut app = APP.lock().unwrap();
                    
                    if app.show_overlay {
                        // Dismiss overlay
                        app.show_overlay = false;
                        if let Some(ctx) = &app.egui_ctx {
                            ctx.request_repaint();
                        }
                        continue;
                    }
                    
                    // Release lock before capturing
                    drop(app);

                    match get_screen_geometry() {
                        Ok(screens) => {
                            let mut app = APP.lock().unwrap();
                            // No original screenshot yet
                            app.original_screenshot = None;
                            app.screens = screens;
                            app.show_overlay = true;
                            if let Some(ctx) = &app.egui_ctx {
                                ctx.request_repaint();
                            }
                        }
                        Err(e) => println!("Geometry error: {}", e),
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });
}

pub fn make_window_transparent(title: &str) {
    unsafe {
        let pool = NSAutoreleasePool::new(nil);
        let app = NSApp();
        let windows: cocoa::base::id = msg_send![app, windows];
        let count: u64 = msg_send![windows, count];
        
        // println!("DEBUG: Found {} windows. Looking for '{}'", count, title);

        for i in 0..count {
            let window: cocoa::base::id = msg_send![windows, objectAtIndex: i];
            let window_title: cocoa::base::id = msg_send![window, title];
            let window_title_str = CStr::from_ptr(window_title.UTF8String()).to_string_lossy();
            
            // println!("DEBUG: Window {}: '{}'", i, window_title_str);

            if window_title_str == title {
                // println!("DEBUG: Found target window! Applying transparency...");
                
                // 1. Set Style Mask: Borderless + FullSizeContentView
                // NSWindowStyleMaskBorderless = 0
                // NSWindowStyleMaskFullSizeContentView = 1 << 15 (32768)
                let style_mask: u64 = 0 | (1 << 15);
                let _: () = msg_send![window, setStyleMask: style_mask];

                // 2. Set Opaque NO
                let _: () = msg_send![window, setOpaque: NO];
                
                // 3. Set Background Color Clear
                let clear_color = NSColor::clearColor(nil);
                let _: () = msg_send![window, setBackgroundColor: clear_color];
                
                // 4. Set Has Shadow NO
                let _: () = msg_send![window, setHasShadow: NO];
                
                // 5. Set Level to Floating (3) or Status (25)
                // NSFloatingWindowLevel = 3
                let level: i32 = 3; 
                let _: () = msg_send![window, setLevel: level];

                // 6. Set Collection Behavior: CanJoinAllSpaces (1 << 0) | FullScreenAuxiliary (1 << 4)
                let collection_behavior: u64 = (1 << 0) | (1 << 4);
                let _: () = msg_send![window, setCollectionBehavior: collection_behavior];

                // Force invalidate shadow
                let _: () = msg_send![window, invalidateShadow];

                // 7. Handle Content View Transparency
                let content_view: cocoa::base::id = msg_send![window, contentView];
                if !content_view.is_null() {
                    let _: () = msg_send![content_view, setWantsLayer: YES];
                    let layer: cocoa::base::id = msg_send![content_view, layer];
                    if !layer.is_null() {
                       // println!("DEBUG: Found layer, setting opaque to NO");
                       let _: () = msg_send![layer, setOpaque: NO];
                       
                       // Optional: Set background color to clear if needed, but setOpaque:NO is most important
                       // let clear_color = NSColor::clearColor(nil);
                       // let cg_color: cocoa::base::id = msg_send![clear_color, CGColor];
                       // let _: () = msg_send![layer, setBackgroundColor: cg_color];
                    }
                }
            }
        }
        pool.drain();
    }
}

fn get_menu_bar_height() -> f64 {
    unsafe {
        let screens = NSScreen::screens(nil);
        // The first screen in the array is always the one with the menu bar (at index 0)
        let main_screen = screens.objectAtIndex(0);
        let frame = NSScreen::frame(main_screen);
        let visible_frame = NSScreen::visibleFrame(main_screen);
        
        // Calculate menu bar height: frame height - (visible frame top)
        // visibleFrame.origin.y is the bottom of the visible area.
        // visibleFrame.size.height is the height of the visible area.
        // So visibleFrame.origin.y + visibleFrame.size.height is the top of the visible area.
        // frame.size.height is the top of the screen.
        let top_inset = frame.size.height - (visible_frame.origin.y + visible_frame.size.height);
        
        // Ensure we don't return negative values or crazy numbers
        if top_inset > 0.0 && top_inset < 100.0 {
            top_inset
        } else {
            24.0 // Fallback
        }
    }
}



use crate::ScreenRect;

pub fn get_screen_geometry() -> Result<Vec<ScreenRect>> {
    let screens = Screen::all().map_err(|e| anyhow::anyhow!("Failed to list screens: {}", e))?;
    
    if screens.is_empty() {
        return Err(anyhow::anyhow!("No screens found"));
    }

    let menu_bar_height_logical = get_menu_bar_height();
    let mut screen_rects = Vec::new();

    for screen in &screens {
        let mut y = screen.display_info.y;
        let mut h_logical = (screen.display_info.height as f32 / screen.display_info.scale_factor) as i32;
        
        // Adjust for menu bar on primary screen (usually at y=0)
        if y == 0 {
            y += menu_bar_height_logical as i32;
            h_logical -= menu_bar_height_logical as i32;
        }

        let x = screen.display_info.x;
        let w_logical = (screen.display_info.width as f32 / screen.display_info.scale_factor) as i32;

        screen_rects.push(ScreenRect {
            x,
            y,
            width_logical: w_logical,
            height_logical: h_logical,
            scale_factor: screen.display_info.scale_factor,
        });
    }

    Ok(screen_rects)
}

pub fn copy_to_clipboard(text: &str) -> Result<()> {
    unsafe {
        let pool = NSAutoreleasePool::new(nil);
        
        let pasteboard: cocoa::base::id = msg_send![class!(NSPasteboard), generalPasteboard];
        let _: () = msg_send![pasteboard, clearContents];
        
        let ns_string = NSString::alloc(nil).init_str(text);
        let objects = NSArray::arrayWithObject(nil, ns_string);
        
        let _: bool = msg_send![pasteboard, writeObjects: objects];
        
        pool.drain();
    }
    Ok(())
}

pub fn capture_area(rect: (i32, i32, u32, u32)) -> Result<ImageBuffer<image::Rgba<u8>, Vec<u8>>> {
    let screens = Screen::all().map_err(|e| anyhow::anyhow!("Failed to list screens: {}", e))?;
    let (x, y, w, h) = rect;
    
    // Find the screen that contains the center of the rect
    let center_x = x + (w as i32 / 2);
    let center_y = y + (h as i32 / 2);
    
    let target_screen = screens.iter().find(|s| {
        let s_x = s.display_info.x;
        let s_y = s.display_info.y;
        let s_w = (s.display_info.width as f32 / s.display_info.scale_factor) as i32;
        let s_h = (s.display_info.height as f32 / s.display_info.scale_factor) as i32;
        
        center_x >= s_x && center_x < s_x + s_w &&
        center_y >= s_y && center_y < s_y + s_h
    });

    if let Some(screen) = target_screen {
        // Convert global logical coordinates to screen-local physical coordinates
        let local_x_logical = x - screen.display_info.x;
        let local_y_logical = y - screen.display_info.y;
        
        let scale = screen.display_info.scale_factor;
        let local_x_px = (local_x_logical as f32 * scale) as i32;
        let local_y_px = (local_y_logical as f32 * scale) as i32;
        let w_px = (w as f32 * scale) as u32;
        let h_px = (h as f32 * scale) as u32;
        
        // Capture area
        let image = screen.capture_area(local_x_px, local_y_px, w_px, h_px)
            .map_err(|e| anyhow::anyhow!("Failed to capture area: {}", e))?;
            
        Ok(image)
    } else {
        Err(anyhow::anyhow!("Selection is outside of any screen"))
    }
}
