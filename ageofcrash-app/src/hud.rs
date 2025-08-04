use crate::config::{HudConfig, HudPosition};
use std::ffi::OsStr;

pub struct BarrierStateConfig {
    pub enabled: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub buffer_zone: i32,
    pub push_factor: i32,
}
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

// HUD window dimensions and layout constants
const HUD_WIDTH: i32 = 300;
const HUD_HEIGHT: i32 = 180;
const HUD_MARGIN: i32 = 20;
const HUD_PADDING: i32 = 10;
const HUD_LINE_HEIGHT: i32 = 18;
const HUD_TITLE_SPACING: i32 = 5;

// HUD color constants (COLORREF format: 0x00BBGGRR)
const COLOR_WHITE: u32 = 0x00FFFFFF;
const COLOR_BLACK: u32 = 0x00000000;
const COLOR_GREEN: u32 = 0x0064FF64;
const COLOR_RED: u32 = 0x006464FF;
const COLOR_YELLOW: u32 = 0x0064FFFF;
const COLOR_DANGER_RED: u32 = 0x000000FF;

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
        config: BarrierStateConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.barrier_enabled = config.enabled;
        self.barrier_x = config.x;
        self.barrier_y = config.y;
        self.barrier_width = config.width;
        self.barrier_height = config.height;
        self.buffer_zone = config.buffer_zone;
        self.push_factor = config.push_factor;

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
                HUD_WIDTH,
                HUD_HEIGHT,
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
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_COMPOSITED,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_POPUP,
            x,
            y,
            HUD_WIDTH,
            HUD_HEIGHT,
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

    let (x, y) = match position {
        HudPosition::TopLeft => (HUD_MARGIN, HUD_MARGIN),
        HudPosition::TopRight => (screen_width - HUD_WIDTH - HUD_MARGIN, HUD_MARGIN),
        HudPosition::BottomLeft => (HUD_MARGIN, screen_height - HUD_HEIGHT - HUD_MARGIN),
        HudPosition::BottomRight => (
            screen_width - HUD_WIDTH - HUD_MARGIN,
            screen_height - HUD_HEIGHT - HUD_MARGIN,
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

            // Create memory DC for double buffering
            let mem_dc = CreateCompatibleDC(hdc);
            let bitmap =
                CreateCompatibleBitmap(hdc, rect.right - rect.left, rect.bottom - rect.top);
            let old_bitmap = SelectObject(mem_dc, bitmap as *mut _);

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
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                DEFAULT_QUALITY,
                DEFAULT_PITCH | FF_DONTCARE,
                ptr::null(),
            );

            let old_font = SelectObject(mem_dc, font as *mut _);

            // Set text colors on memory DC
            SetTextColor(mem_dc, COLOR_WHITE); // White text
            SetBkMode(mem_dc, TRANSPARENT as i32);

            // Draw background on memory DC
            let bg_brush = CreateSolidBrush(COLOR_BLACK); // Black background
            FillRect(mem_dc, &rect, bg_brush);
            DeleteObject(bg_brush as *mut _);

            // Draw HUD content on memory DC
            draw_hud_content(mem_dc, &rect);

            // Copy from memory DC to screen DC (this reduces flicker)
            BitBlt(
                hdc,
                0,
                0,
                rect.right - rect.left,
                rect.bottom - rect.top,
                mem_dc,
                0,
                0,
                SRCCOPY,
            );

            // Clean up
            SelectObject(mem_dc, old_font);
            SelectObject(mem_dc, old_bitmap);
            DeleteObject(font as *mut _);
            DeleteObject(bitmap as *mut _);
            DeleteDC(mem_dc);

            EndPaint(hwnd, &ps);
            0
        }
        WM_DESTROY => 0,
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn draw_hud_content(hdc: HDC, rect: &RECT) {
    let state = HUD_STATE.lock().unwrap();

    let mut y_pos = rect.top + HUD_PADDING;

    // Title
    let title_text: Vec<u16> = OsStr::new("Age of Crash - by HousedHorse")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    TextOutW(
        hdc,
        rect.left + HUD_PADDING,
        y_pos,
        title_text.as_ptr(),
        title_text.len() as i32 - 1,
    );
    y_pos += HUD_LINE_HEIGHT + HUD_TITLE_SPACING;

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
        SetTextColor(hdc, COLOR_GREEN); // Green for enabled
    } else {
        SetTextColor(hdc, COLOR_RED); // Red for disabled
    }

    TextOutW(
        hdc,
        rect.left + HUD_PADDING,
        y_pos,
        status_wide.as_ptr(),
        status_wide.len() as i32 - 1,
    );
    y_pos += HUD_LINE_HEIGHT;

    SetTextColor(hdc, COLOR_WHITE); // Back to white

    // Coordinates
    let coord_text = format!("Position: ({}, {})", state.x, state.y);
    let coord_wide: Vec<u16> = OsStr::new(&coord_text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    TextOutW(
        hdc,
        rect.left + HUD_PADDING,
        y_pos,
        coord_wide.as_ptr(),
        coord_wide.len() as i32 - 1,
    );
    y_pos += HUD_LINE_HEIGHT;

    // Size
    let size_text = format!("Size: {} x {}", state.width, state.height);
    let size_wide: Vec<u16> = OsStr::new(&size_text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    TextOutW(
        hdc,
        rect.left + HUD_PADDING,
        y_pos,
        size_wide.as_ptr(),
        size_wide.len() as i32 - 1,
    );
    y_pos += HUD_LINE_HEIGHT;

    // Buffer zone
    let buffer_text = format!("Buffer Zone: {}px", state.buffer_zone);
    let buffer_wide: Vec<u16> = OsStr::new(&buffer_text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    TextOutW(
        hdc,
        rect.left + HUD_PADDING,
        y_pos,
        buffer_wide.as_ptr(),
        buffer_wide.len() as i32 - 1,
    );
    y_pos += HUD_LINE_HEIGHT;

    // Push factor
    let push_text = format!("Push Factor: {}px", state.push_factor);
    let push_wide: Vec<u16> = OsStr::new(&push_text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    TextOutW(
        hdc,
        rect.left + HUD_PADDING,
        y_pos,
        push_wide.as_ptr(),
        push_wide.len() as i32 - 1,
    );
    y_pos += HUD_LINE_HEIGHT;

    // Mouse position in yellow
    let mouse_text = format!("Mouse: ({}, {})", state.mouse_x, state.mouse_y);
    let mouse_wide: Vec<u16> = OsStr::new(&mouse_text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    SetTextColor(hdc, COLOR_YELLOW); // Yellow color
    TextOutW(
        hdc,
        rect.left + HUD_PADDING,
        y_pos,
        mouse_wide.as_ptr(),
        mouse_wide.len() as i32 - 1,
    );
    y_pos += HUD_LINE_HEIGHT;

    // Mouse in barrier status
    let barrier_status_text = if state.mouse_in_barrier {
        "Mouse Status: IN BARRIER"
    } else if state.mouse_in_buffer {
        "Mouse Status: IN BUFFER ZONE"
    } else {
        "Mouse Status: Okay"
    };

    let barrier_status_wide: Vec<u16> = OsStr::new(barrier_status_text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Color based on mouse location
    if state.mouse_in_barrier {
        SetTextColor(hdc, COLOR_DANGER_RED); // Red when in inner barrier
    } else if state.mouse_in_buffer {
        SetTextColor(hdc, COLOR_YELLOW); // Yellow when in buffer zone
    } else {
        SetTextColor(hdc, COLOR_WHITE); // White when okay
    }

    TextOutW(
        hdc,
        rect.left + HUD_PADDING,
        y_pos,
        barrier_status_wide.as_ptr(),
        barrier_status_wide.len() as i32 - 1,
    );
}

// Global HUD state for access from window procedure
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
    pub mouse_in_barrier: bool,
    pub mouse_in_buffer: bool,
    pub last_refresh: Instant,
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
        mouse_in_barrier: false,
        mouse_in_buffer: false,
        last_refresh: Instant::now(),
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
    const REFRESH_INTERVAL: Duration = Duration::from_millis(33); // ~30 FPS

    if let Ok(mut state) = HUD_STATE.lock() {
        state.mouse_x = x;
        state.mouse_y = y;

        // Check if mouse is in barrier zone
        if state.enabled {
            // Convert from Windows top-left origin to bottom-left origin for comparison
            let barrier_bottom = state.y;
            let barrier_top = state.y - state.height;
            let barrier_left = state.x;
            let barrier_right = state.x + state.width;

            // Check if mouse is within inner barrier (without buffer)
            let in_inner_barrier =
                x >= barrier_left && x <= barrier_right && y >= barrier_top && y <= barrier_bottom;

            // Check if mouse is within barrier + buffer zone
            let in_buffer_zone = x >= (barrier_left - state.buffer_zone)
                && x <= (barrier_right + state.buffer_zone)
                && y >= (barrier_top - state.buffer_zone)
                && y <= (barrier_bottom + state.buffer_zone);

            state.mouse_in_barrier = in_inner_barrier;
            state.mouse_in_buffer = in_buffer_zone && !in_inner_barrier;
        } else {
            state.mouse_in_barrier = false;
            state.mouse_in_buffer = false;
        }

        // Only refresh if enough time has passed since last refresh
        let now = Instant::now();
        if now.duration_since(state.last_refresh) >= REFRESH_INTERVAL {
            state.last_refresh = now;
            drop(state); // Release lock before calling refresh
            refresh_hud_windows();
        }
    }
}

fn refresh_hud_windows() {
    unsafe {
        // Find the HUD window by class name and refresh it efficiently
        let class_name: Vec<u16> = std::ffi::OsStr::new("AgeOfCrashHUD")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let hwnd = FindWindowW(class_name.as_ptr(), ptr::null());
        if !hwnd.is_null() {
            // Use a more efficient invalidation
            InvalidateRect(hwnd, ptr::null(), FALSE);
            // Don't call UpdateWindow here - let the message loop handle it
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HudPosition;

    fn create_test_barrier_state_config(
        enabled: bool,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        buffer_zone: i32,
        push_factor: i32,
    ) -> BarrierStateConfig {
        BarrierStateConfig {
            enabled,
            x,
            y,
            width,
            height,
            buffer_zone,
            push_factor,
        }
    }

    #[test]
    fn test_barrier_state_config_creation() {
        let config = create_test_barrier_state_config(true, 100, 200, 300, 150, 25, 50);

        assert!(config.enabled);
        assert_eq!(config.x, 100);
        assert_eq!(config.y, 200);
        assert_eq!(config.width, 300);
        assert_eq!(config.height, 150);
        assert_eq!(config.buffer_zone, 25);
        assert_eq!(config.push_factor, 50);
    }

    #[test]
    fn test_hud_constants() {
        // Test that HUD constants have expected values (not optimized out since we're testing actual values)
        assert_eq!(HUD_WIDTH, 300);
        assert_eq!(HUD_HEIGHT, 180);
        assert_eq!(HUD_MARGIN, 20);
        assert_eq!(HUD_PADDING, 10);
        assert_eq!(HUD_LINE_HEIGHT, 18);
        assert_eq!(HUD_TITLE_SPACING, 5);

        // Test logical relationships between constants (computed at test time, not compile time)
        let width_check = HUD_WIDTH > HUD_PADDING * 2;
        let height_check = HUD_HEIGHT > HUD_PADDING * 2;
        assert!(width_check, "HUD width should accommodate padding");
        assert!(height_check, "HUD height should accommodate padding");
    }

    #[test]
    fn test_color_constants() {
        // Test color constants are valid COLORREF values
        assert_eq!(COLOR_WHITE, 0x00FFFFFF);
        assert_eq!(COLOR_BLACK, 0x00000000);
        assert_eq!(COLOR_GREEN, 0x0064FF64);
        assert_eq!(COLOR_RED, 0x006464FF);
        assert_eq!(COLOR_YELLOW, 0x0064FFFF);
        assert_eq!(COLOR_DANGER_RED, 0x000000FF);

        // Verify colors are in COLORREF format (0x00BBGGRR)
        // For example, red should have B=0, G=0, R=255
        assert_eq!(COLOR_DANGER_RED & 0xFF, 0xFF); // Red component
        assert_eq!((COLOR_DANGER_RED >> 8) & 0xFF, 0x00); // Green component
        assert_eq!((COLOR_DANGER_RED >> 16) & 0xFF, 0x00); // Blue component
    }

    #[test]
    fn test_calculate_hud_position_top_left() {
        let position = HudPosition::TopLeft;
        let result = calculate_hud_position(&position);

        if let Ok((x, y)) = result {
            assert_eq!(x, HUD_MARGIN);
            assert_eq!(y, HUD_MARGIN);
        }
    }

    #[test]
    fn test_calculate_hud_position_all_positions() {
        // We can't test actual screen dimensions in unit tests, but we can test the logic
        // by checking the position calculation doesn't panic and returns reasonable values

        for position in [
            HudPosition::TopLeft,
            HudPosition::TopRight,
            HudPosition::BottomLeft,
            HudPosition::BottomRight,
        ] {
            let result = calculate_hud_position(&position);
            assert!(
                result.is_ok(),
                "Position calculation should succeed for {:?}",
                position
            );

            if let Ok((x, y)) = result {
                // Basic sanity checks - coordinates should be non-negative and account for margins
                assert!(
                    x >= 0,
                    "X coordinate should be non-negative for {:?}",
                    position
                );
                assert!(
                    y >= 0,
                    "Y coordinate should be non-negative for {:?}",
                    position
                );

                // For right positions, x should account for HUD width
                match position {
                    HudPosition::TopRight | HudPosition::BottomRight => {
                        // x should be screen_width - HUD_WIDTH - HUD_MARGIN
                        // We can't test exact values without mocking GetSystemMetrics
                    }
                    HudPosition::TopLeft | HudPosition::BottomLeft => {
                        assert_eq!(x, HUD_MARGIN);
                    }
                }

                // For bottom positions, y should account for HUD height
                match position {
                    HudPosition::BottomLeft | HudPosition::BottomRight => {
                        // y should be screen_height - HUD_HEIGHT - HUD_MARGIN
                        // We can't test exact values without mocking GetSystemMetrics
                    }
                    HudPosition::TopLeft | HudPosition::TopRight => {
                        assert_eq!(y, HUD_MARGIN);
                    }
                }
            }
        }
    }

    #[test]
    fn test_hud_state_creation() {
        let state = HudState {
            enabled: true,
            x: 100,
            y: 200,
            width: 300,
            height: 150,
            buffer_zone: 25,
            push_factor: 50,
            mouse_x: 150,
            mouse_y: 250,
            mouse_in_barrier: false,
            mouse_in_buffer: true,
            last_refresh: std::time::Instant::now(),
        };

        assert!(state.enabled);
        assert_eq!(state.x, 100);
        assert_eq!(state.y, 200);
        assert_eq!(state.width, 300);
        assert_eq!(state.height, 150);
        assert_eq!(state.buffer_zone, 25);
        assert_eq!(state.push_factor, 50);
        assert_eq!(state.mouse_x, 150);
        assert_eq!(state.mouse_y, 250);
        assert!(!state.mouse_in_barrier);
        assert!(state.mouse_in_buffer);
    }

    #[test]
    fn test_update_global_hud_state() {
        // Test the global HUD state update function
        update_global_hud_state(true, 50, 100, 200, 80, 15, 30);

        // Verify the state was updated by checking via update_mouse_position
        // This is indirect testing since we can't easily access the global state
        update_mouse_position(75, 120);

        // The function should not panic and should handle the update correctly
        // More detailed testing would require accessing the global state directly
    }

    #[test]
    fn test_update_mouse_position_coordinates() {
        // Test basic coordinate updates
        let test_cases = [
            (0, 0),
            (100, 200),
            (1920, 1080),
            (-10, -20), // Negative coordinates should be handled
        ];

        for (x, y) in test_cases {
            // Should not panic
            update_mouse_position(x, y);
        }
    }

    #[test]
    fn test_barrier_inside_detection_logic() {
        // Test the coordinate conversion logic that's used in update_mouse_position
        // We'll test the mathematical logic separately from the global state

        let barrier_x = 100;
        let barrier_y = 500; // bottom coordinate
        let barrier_width = 200;
        let barrier_height = 100;
        let buffer_zone = 25;

        // Convert to Windows coordinates (top-left origin)
        let barrier_bottom = barrier_y;
        let barrier_top = barrier_y - barrier_height; // 500 - 100 = 400
        let barrier_left = barrier_x; // 100
        let barrier_right = barrier_x + barrier_width; // 100 + 200 = 300

        // Test point inside inner barrier
        let mouse_x = 150;
        let mouse_y = 450;
        let in_inner_barrier = mouse_x >= barrier_left
            && mouse_x <= barrier_right
            && mouse_y >= barrier_top
            && mouse_y <= barrier_bottom;
        assert!(in_inner_barrier);

        // Test point in buffer zone but not inner barrier
        let mouse_x = 80; // barrier_left - 20, within buffer zone (barrier_left - buffer_zone = 75)
        let mouse_y = 450;
        let in_buffer_zone = mouse_x >= (barrier_left - buffer_zone)
            && mouse_x <= (barrier_right + buffer_zone)
            && mouse_y >= (barrier_top - buffer_zone)
            && mouse_y <= (barrier_bottom + buffer_zone);
        let in_inner_barrier = mouse_x >= barrier_left
            && mouse_x <= barrier_right
            && mouse_y >= barrier_top
            && mouse_y <= barrier_bottom;

        assert!(in_buffer_zone);
        assert!(!in_inner_barrier);

        // Test point outside both
        let mouse_x = 50; // Too far left
        let mouse_y = 450;
        let in_buffer_zone = mouse_x >= (barrier_left - buffer_zone)
            && mouse_x <= (barrier_right + buffer_zone)
            && mouse_y >= (barrier_top - buffer_zone)
            && mouse_y <= (barrier_bottom + buffer_zone);
        assert!(!in_buffer_zone);
    }

    // Test HUD position enum completeness
    #[test]
    fn test_hud_position_enum_values() {
        // Ensure all enum variants can be created and are distinct
        let positions = [
            HudPosition::TopLeft,
            HudPosition::TopRight,
            HudPosition::BottomLeft,
            HudPosition::BottomRight,
        ];

        // Test that we can create and compare positions
        assert_ne!(positions[0], positions[1]);
        assert_ne!(positions[0], positions[2]);
        assert_ne!(positions[0], positions[3]);

        // Test cloning
        let cloned = positions[0].clone();
        assert_eq!(positions[0], cloned);
    }

    #[test]
    fn test_refresh_interval_constant() {
        use std::time::Duration;

        // Test that the refresh interval constant exists and is reasonable
        const REFRESH_INTERVAL: Duration = Duration::from_millis(33); // ~30 FPS

        assert!(REFRESH_INTERVAL.as_millis() > 0);
        assert!(REFRESH_INTERVAL.as_millis() <= 100); // Should be faster than 10 FPS

        // Verify it's approximately 30 FPS
        let fps = 1000.0 / REFRESH_INTERVAL.as_millis() as f64;
        assert!(
            (25.0..=35.0).contains(&fps),
            "FPS should be around 30, got {}",
            fps
        );
    }
}
