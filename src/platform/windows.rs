use std::sync::{Arc, Mutex};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::System::Threading::*;
use windows::core::*;
use lazy_static::lazy_static;
use image::ImageBuffer;
use crate::{APP, overlay};

// Global event for inter-process restore signaling (manual-reset event)
lazy_static! {
    pub static ref RESTORE_EVENT: Option<windows::Win32::Foundation::HANDLE> = unsafe {
        CreateEventW(None, true, false, w!("ScreenGroundedTranslatorRestoreEvent")).ok()
    };
}

pub struct PlatformHandle {
    _mutex: Option<HANDLE>,
}

pub fn setup_platform() -> anyhow::Result<Option<PlatformHandle>> {
    // Ensure the named event exists
    let _ = RESTORE_EVENT.as_ref();
    
    // Keep the handle alive for the duration of the program
    let mutex = unsafe {
        let instance = CreateMutexW(None, true, w!("ScreenGroundedTranslatorSingleInstanceMutex"));
        if let Ok(handle) = instance {
            if GetLastError() == ERROR_ALREADY_EXISTS {
                // Another instance is running - signal it to restore
                if let Some(event) = RESTORE_EVENT.as_ref() {
                    let _ = SetEvent(*event);
                }
                return Ok(None); // Signal to exit
            }
            Some(handle)
        } else {
            None
        }
    };

    unsafe { let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2); }

    Ok(Some(PlatformHandle { _mutex: mutex }))
}

pub fn spawn_hotkey_listener() {
    std::thread::spawn(|| {
        run_hotkey_listener();
    });
}

fn run_hotkey_listener() {
    unsafe {
        let instance = GetModuleHandleW(None).unwrap();
        let class_name = w!("HotkeyListenerClass");
        
        let wc = WNDCLASSW {
            lpfnWndProc: Some(hotkey_proc),
            hInstance: instance,
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc);
        
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("Listener"),
            WS_OVERLAPPEDWINDOW,
            0, 0, 0, 0,
            None, None, instance, None
        );

        let current_hotkey = APP.lock().unwrap().config.hotkey_code;
        RegisterHotKey(hwnd, 1, HOT_KEY_MODIFIERS(0), current_hotkey);

        let mut msg = MSG::default();
        loop {
            if GetMessageW(&mut msg, None, 0, 0).into() {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

unsafe extern "system" fn hotkey_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            if wparam.0 == 1 {
                // Check if selection overlay is already active, dismiss it instead of opening a new one
                if overlay::is_selection_overlay_active_and_dismiss() {
                    // Successfully dismissed the active overlay, don't create a new one
                    return LRESULT(0);
                }
                
                // No overlay active, proceed with normal capture flow
                match capture_full_screen() {
                    Ok(img) => {
                        {
                            let mut app = APP.lock().unwrap();
                            app.original_screenshot = Some(img);
                        }
                        std::thread::spawn(|| {
                           overlay::show_selection_overlay(); 
                        });
                    },
                    Err(e) => println!("Capture Error: {}", e),
                }
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn capture_full_screen() -> anyhow::Result<ImageBuffer<image::Rgba<u8>, Vec<u8>>> {
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        let hdc_screen = GetDC(None);
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
        SelectObject(hdc_mem, hbitmap);

        BitBlt(hdc_mem, 0, 0, width, height, hdc_screen, x, y, SRCCOPY).ok()?;

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0 as u32,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut buffer: Vec<u8> = vec![0; (width * height * 4) as usize];
        GetDIBits(hdc_mem, hbitmap, 0, height as u32, Some(buffer.as_mut_ptr() as *mut _), &mut bmi, DIB_RGB_COLORS);

        for chunk in buffer.chunks_exact_mut(4) {
            chunk.swap(0, 2);
            chunk[3] = 255;
        }

        DeleteObject(hbitmap);
        DeleteDC(hdc_mem);
        ReleaseDC(None, hdc_screen);

        let img = ImageBuffer::from_raw(width as u32, height as u32, buffer)
            .ok_or_else(|| anyhow::anyhow!("Buffer creation failed"))?;
        
        Ok(img)
    }
}
