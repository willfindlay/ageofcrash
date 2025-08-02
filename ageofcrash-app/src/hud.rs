use crate::config::{HudConfig, HudPosition};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

pub struct Hud {
    hwnd: HWND,
    config: HudConfig,
    enabled: bool,
    barrier_enabled: bool,
    barrier_x: i32,
    barrier_y: i32,
    barrier_width: i32,
    barrier_height: i32,
    buffer_zone: i32,
    push_factor: i32,
}

impl Hud {
    pub fn new(config: HudConfig) -> Result<Self, Box<dyn std::error::Error>> {
        if !config.enabled {
            return Ok(Self {
                hwnd: ptr::null_mut(),
                config,
                enabled: false,
                barrier_enabled: false,
                barrier_x: 0,
                barrier_y: 0,
                barrier_width: 0,
                barrier_height: 0,
                buffer_zone: 0,
                push_factor: 0,
            });
        }

        let hwnd = create_hud_window(&config)?;

        Ok(Self {
            hwnd,
            config,
            enabled: true,
            barrier_enabled: false,
            barrier_x: 0,
            barrier_y: 0,
            barrier_width: 0,
            barrier_height: 0,
            buffer_zone: 0,
            push_factor: 0,
        })
    }

    pub fn update_config(
        &mut self,
        new_config: HudConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if new_config.enabled && !self.enabled {
            // Create window if it doesn't exist
            self.hwnd = create_hud_window(&new_config)?;
            self.enabled = true;
        } else if !new_config.enabled && self.enabled {
            // Destroy window if it exists
            if !self.hwnd.is_null() {
                unsafe {
                    DestroyWindow(self.hwnd);
                }
                self.hwnd = ptr::null_mut();
            }
            self.enabled = false;
        } else if self.enabled {
            // Update existing window position if needed
            self.update_position(&new_config)?;
        }

        self.config = new_config;

        if self.enabled {
            self.refresh_display()?;
        }

        Ok(())
    }

    pub fn update_barrier_state(
        &mut self,
        enabled: bool,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        buffer_zone: i32,
        push_factor: i32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.barrier_enabled = enabled;
        self.barrier_x = x;
        self.barrier_y = y;
        self.barrier_width = width;
        self.barrier_height = height;
        self.buffer_zone = buffer_zone;
        self.push_factor = push_factor;

        if self.enabled {
            self.refresh_display()?;
        }

        Ok(())
    }

    fn update_position(&self, config: &HudConfig) -> Result<(), Box<dyn std::error::Error>> {
        if self.hwnd.is_null() {
            return Ok(());
        }

        let (x, y) = calculate_hud_position(&config.position)?;

        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                x,
                y,
                300, // Width
                150, // Height
                SWP_NOACTIVATE | SWP_NOOWNERZORDER,
            );
        }

        Ok(())
    }

    fn refresh_display(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.hwnd.is_null() {
            return Ok(());
        }

        unsafe {
            InvalidateRect(self.hwnd, ptr::null(), TRUE);
            UpdateWindow(self.hwnd);
        }

        Ok(())
    }
}

impl Drop for Hud {
    fn drop(&mut self) {
        if !self.hwnd.is_null() {
            unsafe {
                DestroyWindow(self.hwnd);
            }
        }
    }
}

fn create_hud_window(config: &HudConfig) -> Result<HWND, Box<dyn std::error::Error>> {
    let class_name: Vec<u16> = OsStr::new("AgeOfCrashHUD")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let window_title: Vec<u16> = OsStr::new("Mouse Barrier HUD")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Register window class
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(hud_window_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: unsafe { GetModuleHandleW(ptr::null()) },
        hIcon: ptr::null_mut(),
        hCursor: unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) },
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
    };

    unsafe {
        RegisterClassW(&wc);
    }

    let (x, y) = calculate_hud_position(&config.position)?;

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_POPUP,
            x,
            y,
            300, // Width
            150, // Height
            ptr::null_mut(),
            ptr::null_mut(),
            GetModuleHandleW(ptr::null()),
            ptr::null_mut(),
        )
    };

    if hwnd.is_null() {
        return Err("Failed to create HUD window".into());
    }

    // Set window transparency
    unsafe {
        SetLayeredWindowAttributes(hwnd, 0, config.background_alpha, LWA_ALPHA);

        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        UpdateWindow(hwnd);
    }

    Ok(hwnd)
}

fn calculate_hud_position(
    position: &HudPosition,
) -> Result<(i32, i32), Box<dyn std::error::Error>> {
    let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };

    let hud_width = 300;
    let hud_height = 150;
    let margin = 20;

    let (x, y) = match position {
        HudPosition::TopLeft => (margin, margin),
        HudPosition::TopRight => (screen_width - hud_width - margin, margin),
        HudPosition::BottomLeft => (margin, screen_height - hud_height - margin),
        HudPosition::BottomRight => (
            screen_width - hud_width - margin,
            screen_height - hud_height - margin,
        ),
    };

    Ok((x, y))
}

unsafe extern "system" fn hud_window_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            // Get window rect
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);

            // Create fonts and brushes
            let font = CreateFontW(
                14,
                0,
                0,
                0,
                FW_NORMAL,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                DEFAULT_QUALITY as u32,
                DEFAULT_PITCH as u32 | FF_DONTCARE as u32,
                ptr::null(),
            );

            let old_font = SelectObject(hdc, font as *mut _);

            // Set text colors
            SetTextColor(hdc, RGB(255, 255, 255)); // White text
            SetBkMode(hdc, TRANSPARENT as i32);

            // Draw background
            let bg_brush = CreateSolidBrush(RGB(0, 0, 0)); // Black background
            FillRect(hdc, &rect, bg_brush);
            DeleteObject(bg_brush as *mut _);

            // Draw HUD content
            draw_hud_content(hdc, &rect);

            SelectObject(hdc, old_font);
            DeleteObject(font as *mut _);
            EndPaint(hwnd, &ps);
            0
        }
        WM_DESTROY => 0,
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn draw_hud_content(hdc: HDC, rect: &RECT) {
    let state = HUD_STATE.lock().unwrap();

    let mut y_pos = rect.top + 10;
    let line_height = 18;

    // Title
    let title_text: Vec<u16> = OsStr::new("Age of Crash - by HousedHorse")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    TextOutW(
        hdc,
        rect.left + 10,
        y_pos,
        title_text.as_ptr(),
        title_text.len() as i32 - 1,
    );
    y_pos += line_height + 5;

    // Status with color coding
    let status_text = if state.enabled {
        "Status: ENABLED"
    } else {
        "Status: DISABLED"
    };

    let status_wide: Vec<u16> = OsStr::new(status_text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Color code based on status
    if state.enabled {
        SetTextColor(hdc, RGB(100, 255, 100)); // Green for enabled
    } else {
        SetTextColor(hdc, RGB(255, 100, 100)); // Red for disabled
    }

    TextOutW(
        hdc,
        rect.left + 10,
        y_pos,
        status_wide.as_ptr(),
        status_wide.len() as i32 - 1,
    );
    y_pos += line_height;

    SetTextColor(hdc, RGB(255, 255, 255)); // Back to white

    // Coordinates
    let coord_text = format!("Position: ({}, {})", state.x, state.y);
    let coord_wide: Vec<u16> = OsStr::new(&coord_text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    TextOutW(
        hdc,
        rect.left + 10,
        y_pos,
        coord_wide.as_ptr(),
        coord_wide.len() as i32 - 1,
    );
    y_pos += line_height;

    // Size
    let size_text = format!("Size: {} x {}", state.width, state.height);
    let size_wide: Vec<u16> = OsStr::new(&size_text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    TextOutW(
        hdc,
        rect.left + 10,
        y_pos,
        size_wide.as_ptr(),
        size_wide.len() as i32 - 1,
    );
    y_pos += line_height;

    // Buffer zone
    let buffer_text = format!("Buffer Zone: {}px", state.buffer_zone);
    let buffer_wide: Vec<u16> = OsStr::new(&buffer_text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    TextOutW(
        hdc,
        rect.left + 10,
        y_pos,
        buffer_wide.as_ptr(),
        buffer_wide.len() as i32 - 1,
    );
    y_pos += line_height;

    // Push factor
    let push_text = format!("Push Factor: {}px", state.push_factor);
    let push_wide: Vec<u16> = OsStr::new(&push_text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    TextOutW(
        hdc,
        rect.left + 10,
        y_pos,
        push_wide.as_ptr(),
        push_wide.len() as i32 - 1,
    );
    y_pos += line_height;

    // Mouse position in yellow
    let mouse_text = format!("Mouse: ({}, {})", state.mouse_x, state.mouse_y);
    let mouse_wide: Vec<u16> = OsStr::new(&mouse_text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    SetTextColor(hdc, RGB(255, 255, 100)); // Yellow color
    TextOutW(
        hdc,
        rect.left + 10,
        y_pos,
        mouse_wide.as_ptr(),
        mouse_wide.len() as i32 - 1,
    );
}

// Global HUD state for access from window procedure
use std::sync::Arc;
use std::sync::Mutex;

pub struct HudState {
    pub enabled: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub buffer_zone: i32,
    pub push_factor: i32,
    pub mouse_x: i32,
    pub mouse_y: i32,
}

lazy_static::lazy_static! {
    static ref HUD_STATE: Arc<Mutex<HudState>> = Arc::new(Mutex::new(HudState {
        enabled: false,
        x: 0,
        y: 0,
        width: 0,
        height: 0,
        buffer_zone: 0,
        push_factor: 0,
        mouse_x: 0,
        mouse_y: 0,
    }));
}

pub fn update_global_hud_state(
    enabled: bool,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    buffer_zone: i32,
    push_factor: i32,
) {
    if let Ok(mut state) = HUD_STATE.lock() {
        state.enabled = enabled;
        state.x = x;
        state.y = y;
        state.width = width;
        state.height = height;
        state.buffer_zone = buffer_zone;
        state.push_factor = push_factor;
    }
}

pub fn update_mouse_position(x: i32, y: i32) {
    if let Ok(mut state) = HUD_STATE.lock() {
        state.mouse_x = x;
        state.mouse_y = y;
    }
    
    // Find and refresh any HUD windows
    refresh_hud_windows();
}

fn refresh_hud_windows() {
    unsafe {
        // Find the HUD window by class name and refresh it
        let class_name: Vec<u16> = std::ffi::OsStr::new("AgeOfCrashHUD")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        
        let hwnd = FindWindowW(class_name.as_ptr(), ptr::null());
        if !hwnd.is_null() {
            InvalidateRect(hwnd, ptr::null(), FALSE);
        }
    }
}
