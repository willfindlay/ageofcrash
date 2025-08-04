use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tracing::{info, warn};
use winapi::shared::minwindef::{LPARAM, LRESULT, TRUE, UINT, WPARAM};
use winapi::shared::windef::{HWND, POINT, RECT};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

static MOUSE_BARRIER_STATE: OnceLock<Arc<Mutex<Option<MouseBarrierState>>>> = OnceLock::new();
static KEYBOARD_CALLBACK: OnceLock<Arc<Mutex<Option<Box<dyn Fn(u32, bool) + Send + Sync>>>>> =
    OnceLock::new();
static MOUSE_POSITION_CALLBACK: OnceLock<Arc<Mutex<Option<Box<dyn Fn(i32, i32) + Send + Sync>>>>> =
    OnceLock::new();
static KEYBOARD_HOOK_HANDLE: AtomicPtr<winapi::shared::windef::HHOOK__> =
    AtomicPtr::new(std::ptr::null_mut());
static MOUSE_HOOK_HANDLE: AtomicPtr<winapi::shared::windef::HHOOK__> =
    AtomicPtr::new(std::ptr::null_mut());
static LAST_IN_BARRIER: AtomicBool = AtomicBool::new(false);
static MIDDLE_BUTTON_MONITORING: AtomicBool = AtomicBool::new(false);
static MIDDLE_MOUSE_DOWN: AtomicBool = AtomicBool::new(false);
static HOOK_INSTALL_REQUESTED: AtomicBool = AtomicBool::new(false);
static HOOK_UNINSTALL_REQUESTED: AtomicBool = AtomicBool::new(false);
static LAST_MOUSE_POS: Mutex<Option<POINT>> = Mutex::new(None);
static OVERLAY_WINDOWS: [AtomicPtr<winapi::shared::windef::HWND__>; 4] = [
    AtomicPtr::new(std::ptr::null_mut()),
    AtomicPtr::new(std::ptr::null_mut()),
    AtomicPtr::new(std::ptr::null_mut()),
    AtomicPtr::new(std::ptr::null_mut()),
];

// Cached screen metrics to avoid repeated API calls
static SCREEN_WIDTH: AtomicI32 = AtomicI32::new(0);
static SCREEN_HEIGHT: AtomicI32 = AtomicI32::new(0);

// Current overlay color for window painting
static CURRENT_OVERLAY_COLOR: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0x00FF0000); // Default red

#[derive(Clone)]
struct MouseBarrierState {
    barrier_rect: RECT,
    buffer_zone: i32,
    push_factor: i32,
    enabled: bool,
    overlay_color: u32, // RGB color as 0x00RRGGBB
    overlay_alpha: u8,  // Alpha transparency (0-255)
}

pub struct MouseBarrier;

pub struct KeyboardHook;

impl MouseBarrier {
    pub fn new(
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        buffer_zone: i32,
        push_factor: i32,
        overlay_color: (u8, u8, u8),
        overlay_alpha: u8,
    ) -> Self {
        // Convert from bottom-left origin to Windows top-left origin
        let barrier_rect = RECT {
            left: x,
            top: y - height,  // y is bottom, so top = y - height
            right: x + width, // right extends from left
            bottom: y,        // bottom is the y coordinate
        };

        let state = MouseBarrierState {
            barrier_rect,
            buffer_zone,
            push_factor,
            enabled: false,
            overlay_color: ((overlay_color.0 as u32) << 16)
                | ((overlay_color.1 as u32) << 8)
                | (overlay_color.2 as u32),
            overlay_alpha,
        };

        let state_lock = MOUSE_BARRIER_STATE.get_or_init(|| Arc::new(Mutex::new(None)));
        *state_lock.lock().unwrap() = Some(state.clone());

        // Cache screen metrics on first initialization
        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);
            SCREEN_WIDTH.store(width, Ordering::Relaxed);
            SCREEN_HEIGHT.store(height, Ordering::Relaxed);
        }

        // Update the global overlay color
        CURRENT_OVERLAY_COLOR.store(state.overlay_color, Ordering::Relaxed);

        Self
    }

    pub fn enable(&mut self) -> Result<(), String> {
        let current_hook = MOUSE_HOOK_HANDLE.load(Ordering::Acquire);
        if !current_hook.is_null() {
            return Ok(());
        }

        let state_lock = MOUSE_BARRIER_STATE.get().unwrap();
        if let Some(ref mut state) = *state_lock.lock().unwrap() {
            state.enabled = true;
        }

        // Create overlay windows (4 rectangles)
        match create_overlay_windows() {
            Ok(windows) => {
                for (i, hwnd) in windows.into_iter().enumerate() {
                    if i < 4 {
                        OVERLAY_WINDOWS[i].store(hwnd, Ordering::Release);
                    }
                }
                info!("Created overlay windows");
            }
            Err(e) => {
                warn!("Failed to create overlay windows: {}", e);
            }
        }

        // Start middle button monitoring that controls hook installation
        MIDDLE_BUTTON_MONITORING.store(true, Ordering::Release);
        thread::spawn(move || {
            monitor_middle_button_and_control_hook();
        });

        // Install main mouse hook initially
        install_mouse_hook()?;

        Ok(())
    }

    pub fn disable(&mut self) -> Result<(), String> {
        // Stop middle button monitoring
        MIDDLE_BUTTON_MONITORING.store(false, Ordering::Release);

        let state_lock = MOUSE_BARRIER_STATE.get().unwrap();
        if let Some(ref mut state) = *state_lock.lock().unwrap() {
            state.enabled = false;
        }

        uninstall_mouse_hook()?;

        // Destroy overlay windows
        for atomic_ptr in &OVERLAY_WINDOWS {
            let hwnd = atomic_ptr.swap(ptr::null_mut(), Ordering::AcqRel);
            if !hwnd.is_null() {
                unsafe {
                    DestroyWindow(hwnd);
                }
            }
        }
        info!("Destroyed overlay windows");

        Ok(())
    }

    pub fn toggle(&mut self) -> Result<bool, String> {
        let is_enabled = self.is_enabled();
        if is_enabled {
            self.disable()?;
            Ok(false)
        } else {
            self.enable()?;
            Ok(true)
        }
    }

    pub fn is_enabled(&self) -> bool {
        let state_lock = MOUSE_BARRIER_STATE.get().unwrap();
        if let Some(ref state) = *state_lock.lock().unwrap() {
            state.enabled
        } else {
            false
        }
    }

    pub fn update_barrier(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        buffer_zone: i32,
        push_factor: i32,
        overlay_color: (u8, u8, u8),
        overlay_alpha: u8,
    ) {
        let state_lock = MOUSE_BARRIER_STATE.get().unwrap();
        if let Some(ref mut state) = *state_lock.lock().unwrap() {
            // Convert from bottom-left origin to Windows top-left origin
            state.barrier_rect = RECT {
                left: x,
                top: y - height,  // y is bottom, so top = y - height
                right: x + width, // right extends from left
                bottom: y,        // bottom is the y coordinate
            };
            state.buffer_zone = buffer_zone;
            state.push_factor = push_factor;
            state.overlay_color = ((overlay_color.0 as u32) << 16)
                | ((overlay_color.1 as u32) << 8)
                | (overlay_color.2 as u32);
            state.overlay_alpha = overlay_alpha;

            // Update the global overlay color
            CURRENT_OVERLAY_COLOR.store(state.overlay_color, Ordering::Relaxed);
        }

        // Update the overlay windows if they exist
        for atomic_ptr in &OVERLAY_WINDOWS {
            let hwnd = atomic_ptr.load(Ordering::Acquire);
            if !hwnd.is_null() {
                unsafe {
                    InvalidateRect(hwnd, ptr::null(), TRUE);
                }
            }
        }
    }
}

impl Drop for MouseBarrier {
    fn drop(&mut self) {
        let _ = self.disable();
    }
}

impl KeyboardHook {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(u32, bool) + Send + Sync + 'static,
    {
        let callback_lock = KEYBOARD_CALLBACK.get_or_init(|| Arc::new(Mutex::new(None)));
        *callback_lock.lock().unwrap() = Some(Box::new(callback));

        // Hook handle will be managed globally via atomic pointer

        Self
    }

    pub fn enable(&mut self) -> Result<(), String> {
        let current_hook = KEYBOARD_HOOK_HANDLE.load(Ordering::Acquire);
        if !current_hook.is_null() {
            return Ok(());
        }

        unsafe {
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_proc),
                GetModuleHandleW(std::ptr::null()),
                0,
            );

            if hook.is_null() {
                return Err(format!("Failed to set keyboard hook: {}", GetLastError()));
            }

            KEYBOARD_HOOK_HANDLE.store(hook, Ordering::Release);
        }

        Ok(())
    }

    pub fn disable(&mut self) -> Result<(), String> {
        let hook = KEYBOARD_HOOK_HANDLE.swap(std::ptr::null_mut(), Ordering::AcqRel);

        if !hook.is_null() {
            unsafe {
                if UnhookWindowsHookEx(hook) == 0 {
                    return Err(format!("Failed to unhook keyboard: {}", GetLastError()));
                }
            }
        }

        Ok(())
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        let _ = self.disable();
    }
}

pub fn set_mouse_position_callback<F>(callback: F)
where
    F: Fn(i32, i32) + Send + Sync + 'static,
{
    let callback_lock = MOUSE_POSITION_CALLBACK.get_or_init(|| Arc::new(Mutex::new(None)));
    if let Ok(mut guard) = callback_lock.lock() {
        *guard = Some(Box::new(callback));
    }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && wparam == WM_MOUSEMOVE as WPARAM {
        let mouse_data = *(lparam as *const MSLLHOOKSTRUCT);
        let current_pos = mouse_data.pt;

        // Update HUD with current mouse position
        if let Some(callback_lock) = MOUSE_POSITION_CALLBACK.get() {
            if let Ok(callback_guard) = callback_lock.lock() {
                if let Some(ref callback) = *callback_guard {
                    callback(current_pos.x, current_pos.y);
                }
            }
        }

        if let Some(state_lock) = MOUSE_BARRIER_STATE.get() {
            if let Ok(state_guard) = state_lock.lock() {
                if let Some(ref state) = *state_guard {
                    if state.enabled {
                        // Get last mouse position for trajectory checking
                        let last_pos = if let Ok(mut last_pos_guard) = LAST_MOUSE_POS.lock() {
                            let last = last_pos_guard.clone();
                            *last_pos_guard = Some(current_pos);
                            last
                        } else {
                            None
                        };

                        // Create buffer zone rect
                        let buffer_rect = RECT {
                            left: state.barrier_rect.left - state.buffer_zone,
                            top: state.barrier_rect.top - state.buffer_zone,
                            right: state.barrier_rect.right + state.buffer_zone,
                            bottom: state.barrier_rect.bottom + state.buffer_zone,
                        };

                        // First, check trajectory for fast movements
                        if let Some(last) = last_pos {
                            if let Some(safe_pos) = check_movement_path(&last, &current_pos, &state.barrier_rect, &buffer_rect) {
                                // Movement would pass through barrier, stop at safe position
                                SetCursorPos(safe_pos.x, safe_pos.y);
                                return 1;
                            }
                            
                            // Predictive positioning - check where cursor is heading
                            let dx = current_pos.x - last.x;
                            let dy = current_pos.y - last.y;
                            let predicted_pos = POINT {
                                x: current_pos.x + dx,
                                y: current_pos.y + dy,
                            };
                            
                            // If predicted position would be in barrier, stop now
                            if point_in_rect(&predicted_pos, &state.barrier_rect) {
                                // Find a safe position just outside the buffer
                                let push_factor = calculate_dynamic_push_factor(state.push_factor, &last, &current_pos);
                                let safe_pos = push_point_out_of_rect(&current_pos, &buffer_rect, push_factor);
                                SetCursorPos(safe_pos.x, safe_pos.y);
                                return 1;
                            }
                        }

                        if point_in_rect(&current_pos, &state.barrier_rect) {
                            warn!(x = current_pos.x, y = current_pos.y, "Cursor in barrier!")
                        }

                        let in_buffer = point_in_rect(&current_pos, &buffer_rect);
                        let was_in_buffer = LAST_IN_BARRIER.load(Ordering::Acquire);

                        if in_buffer != was_in_buffer {
                            LAST_IN_BARRIER.store(in_buffer, Ordering::Release);
                        }

                        if in_buffer {
                            // Calculate dynamic push factor based on movement speed
                            let push_factor = if let Some(last) = last_pos {
                                calculate_dynamic_push_factor(state.push_factor, &last, &current_pos)
                            } else {
                                state.push_factor
                            };

                            let new_pos = push_point_out_of_rect(
                                &current_pos,
                                &buffer_rect,
                                push_factor,
                            );

                            SetCursorPos(new_pos.x, new_pos.y);

                            return 1;
                        }
                    }
                }
            }
        }
    }

    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        if let Some(callback_lock) = KEYBOARD_CALLBACK.get() {
            if let Ok(callback_guard) = callback_lock.lock() {
                if let Some(ref callback) = *callback_guard {
                    let kbd_data = *(lparam as *const KBDLLHOOKSTRUCT);
                    let is_key_down =
                        wparam == WM_KEYDOWN as WPARAM || wparam == WM_SYSKEYDOWN as WPARAM;
                    callback(kbd_data.vkCode, is_key_down);
                }
            }
        }
    }

    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

fn install_mouse_hook() -> Result<(), String> {
    let current_hook = MOUSE_HOOK_HANDLE.load(Ordering::Acquire);
    if !current_hook.is_null() {
        return Ok(());
    }

    unsafe {
        let hook = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(mouse_proc),
            GetModuleHandleW(std::ptr::null()),
            0,
        );

        if hook.is_null() {
            return Err(format!("Failed to set mouse hook: {}", GetLastError()));
        }

        MOUSE_HOOK_HANDLE.store(hook, Ordering::Release);
    }
    Ok(())
}

fn uninstall_mouse_hook() -> Result<(), String> {
    let hook = MOUSE_HOOK_HANDLE.swap(std::ptr::null_mut(), Ordering::AcqRel);

    if !hook.is_null() {
        unsafe {
            if UnhookWindowsHookEx(hook) == 0 {
                return Err(format!("Failed to unhook mouse: {}", GetLastError()));
            }
        }
    }
    Ok(())
}

pub fn process_hook_requests() {
    // Check for uninstall requests
    if HOOK_UNINSTALL_REQUESTED.swap(false, Ordering::AcqRel) {
        if let Err(e) = uninstall_mouse_hook() {
            warn!("Failed to uninstall mouse hook: {}", e);
        } else {
            info!("Uninstalled mouse hook due to middle button press");
        }
    }

    // Check for install requests
    if HOOK_INSTALL_REQUESTED.swap(false, Ordering::AcqRel) {
        if let Err(e) = install_mouse_hook() {
            warn!("Failed to reinstall mouse hook: {}", e);
        } else {
            info!("Reinstalled mouse hook after middle button release");
        }
    }
}

fn monitor_middle_button_and_control_hook() {
    let mut last_middle_state = false;

    while MIDDLE_BUTTON_MONITORING.load(Ordering::Acquire) {
        unsafe {
            let middle_pressed = GetAsyncKeyState(VK_MBUTTON) & 0x8000u16 as i16 != 0;

            // Detect state changes
            if middle_pressed != last_middle_state {
                if middle_pressed {
                    // Middle button pressed - request hook uninstall
                    HOOK_UNINSTALL_REQUESTED.store(true, Ordering::Release);
                    info!("Requested mouse hook uninstall due to middle button press");
                } else {
                    // Middle button released - request hook reinstall if barrier is enabled
                    if let Some(state_lock) = MOUSE_BARRIER_STATE.get() {
                        if let Ok(state_guard) = state_lock.lock() {
                            if let Some(ref state) = *state_guard {
                                if state.enabled {
                                    HOOK_INSTALL_REQUESTED.store(true, Ordering::Release);
                                    info!("Requested mouse hook reinstall after middle button release");
                                }
                            }
                        }
                    }
                }
                last_middle_state = middle_pressed;
            }

            MIDDLE_MOUSE_DOWN.store(middle_pressed, Ordering::Relaxed);
        }
        thread::sleep(Duration::from_millis(5)); // 200Hz polling for responsiveness
    }
}

fn point_in_rect(point: &POINT, rect: &RECT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

fn check_movement_path(start: &POINT, end: &POINT, barrier: &RECT, buffer: &RECT) -> Option<POINT> {
    // Skip if movement is too small
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    if dx.abs() < 2 && dy.abs() < 2 {
        return None;
    }
    
    // Check multiple points along the movement path
    let steps = 10; // More steps for better accuracy
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let check_point = POINT {
            x: (start.x as f32 + dx as f32 * t) as i32,
            y: (start.y as f32 + dy as f32 * t) as i32,
        };
        
        // Check if this intermediate point hits the barrier
        if point_in_rect(&check_point, barrier) {
            // Find the last safe point outside the buffer zone
            for j in (0..i).rev() {
                let safe_t = j as f32 / steps as f32;
                let safe_point = POINT {
                    x: (start.x as f32 + dx as f32 * safe_t) as i32,
                    y: (start.y as f32 + dy as f32 * safe_t) as i32,
                };
                
                if !point_in_rect(&safe_point, buffer) {
                    return Some(safe_point);
                }
            }
            // If no safe point found, return start position
            return Some(*start);
        }
    }
    None
}

fn calculate_dynamic_push_factor(base_factor: i32, last_pos: &POINT, current_pos: &POINT) -> i32 {
    let dx = (current_pos.x - last_pos.x) as f64;
    let dy = (current_pos.y - last_pos.y) as f64;
    let speed = (dx * dx + dy * dy).sqrt();
    
    // Scale push factor: faster movement = larger push
    // Speed 10 = 1x, Speed 50 = 2x, Speed 100+ = 3x
    let multiplier = (speed / 25.0).max(1.0).min(3.0);
    (base_factor as f64 * multiplier) as i32
}

fn push_point_out_of_rect(point: &POINT, rect: &RECT, push_factor: i32) -> POINT {
    // Use cached screen metrics
    let screen_width = SCREEN_WIDTH.load(Ordering::Relaxed);
    let screen_height = SCREEN_HEIGHT.load(Ordering::Relaxed);

    // Determine which edge the mouse is closest to and push away from that edge
    let dist_to_left = point.x - rect.left;
    let dist_to_right = rect.right - point.x;
    let dist_to_top = point.y - rect.top;
    let dist_to_bottom = rect.bottom - point.y;

    // Find the minimum distance to determine which edge to push from
    let min_dist = dist_to_left
        .min(dist_to_right)
        .min(dist_to_top)
        .min(dist_to_bottom);

    let new_point = if min_dist == dist_to_left {
        // Push left, but ensure we don't go below 0
        let target_x = rect.left - push_factor;
        POINT {
            x: if target_x < 0 {
                // If pushing left would go off-screen, push right instead
                rect.right + push_factor
            } else {
                target_x
            },
            y: point.y,
        }
    } else if min_dist == dist_to_right {
        // Push right, but ensure we don't exceed screen width
        let target_x = rect.right + push_factor;
        POINT {
            x: if target_x >= screen_width {
                // If pushing right would go off-screen, push left instead
                (rect.left - push_factor).max(0)
            } else {
                target_x
            },
            y: point.y,
        }
    } else if min_dist == dist_to_top {
        // Push up, but ensure we don't go below 0
        let target_y = rect.top - push_factor;
        POINT {
            x: point.x,
            y: if target_y < 0 {
                // If pushing up would go off-screen, push down instead
                rect.bottom + push_factor
            } else {
                target_y
            },
        }
    } else {
        // Push down, but ensure we don't exceed screen height
        let target_y = rect.bottom + push_factor;
        POINT {
            x: point.x,
            y: if target_y >= screen_height {
                // If pushing down would go off-screen, push up instead
                (rect.top - push_factor).max(0)
            } else {
                target_y
            },
        }
    };

    // Convert from physical coordinates to logical coordinates for SetCursorPos
    // Physical screen: ~3840x2160, Logical screen: varies
    let scale_x = screen_width as f64 / 3840.0;
    let scale_y = screen_height as f64 / 2160.0;

    let logical_x = (new_point.x as f64 * scale_x).round() as i32;
    let logical_y = (new_point.y as f64 * scale_y).round() as i32;

    POINT {
        x: logical_x.clamp(0, screen_width - 1),
        y: logical_y.clamp(0, screen_height - 1),
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            // Draw overlay rectangle with configured color
            let color = CURRENT_OVERLAY_COLOR.load(Ordering::Relaxed);
            let r = ((color >> 16) & 0xFF) as u8;
            let g = ((color >> 8) & 0xFF) as u8;
            let b = (color & 0xFF) as u8;

            let brush = CreateSolidBrush(RGB(r, g, b));
            let mut client_rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            GetClientRect(hwnd, &mut client_rect);
            FillRect(hdc, &client_rect, brush);
            DeleteObject(brush as *mut _);

            EndPaint(hwnd, &ps);
            0
        }
        WM_ERASEBKGND => {
            1 // Return non-zero to indicate we handled it
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn create_overlay_windows() -> Result<Vec<HWND>, String> {
    let state_lock = MOUSE_BARRIER_STATE.get().unwrap();
    let mut windows = Vec::new();

    if let Ok(state_guard) = state_lock.lock() {
        if let Some(ref state) = *state_guard {
            // Calculate positions for 4 windows
            let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
            let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
            let scale_x = screen_width as f64 / 3840.0;
            let scale_y = screen_height as f64 / 2160.0;

            let barrier_left = (state.barrier_rect.left as f64 * scale_x).round() as i32;
            let barrier_top = (state.barrier_rect.top as f64 * scale_y).round() as i32;
            let barrier_right = (state.barrier_rect.right as f64 * scale_x).round() as i32;
            let barrier_bottom = (state.barrier_rect.bottom as f64 * scale_y).round() as i32;

            let scaled_buffer = (state.buffer_zone as f64 * scale_x).round() as i32;
            let buffer_left = barrier_left - scaled_buffer;
            let buffer_top = barrier_top - scaled_buffer;
            let buffer_right = barrier_right + scaled_buffer;
            let buffer_bottom = barrier_bottom + scaled_buffer;

            // Create 4 windows - top, bottom, left, right
            // Clamp to screen boundaries to avoid covering taskbar
            let max_bottom = screen_height - 60; // Leave space for taskbar
            let clamped_buffer_bottom = buffer_bottom.min(max_bottom);
            let clamped_buffer_top = buffer_top.max(0);
            let clamped_buffer_left = buffer_left.max(0);
            let clamped_buffer_right = buffer_right.min(screen_width);

            let window_configs = [
                (
                    "top",
                    clamped_buffer_left,
                    clamped_buffer_top,
                    clamped_buffer_right - clamped_buffer_left,
                    barrier_top - clamped_buffer_top,
                ),
                (
                    "bottom",
                    clamped_buffer_left,
                    barrier_bottom,
                    clamped_buffer_right - clamped_buffer_left,
                    clamped_buffer_bottom - barrier_bottom,
                ),
                (
                    "left",
                    clamped_buffer_left,
                    barrier_top,
                    barrier_left - clamped_buffer_left,
                    barrier_bottom - barrier_top,
                ),
                (
                    "right",
                    barrier_right,
                    barrier_top,
                    clamped_buffer_right - barrier_right,
                    barrier_bottom - barrier_top,
                ),
            ];

            for (name, x, y, width, height) in window_configs.iter() {
                if *width > 0 && *height > 0 {
                    match create_single_overlay_window(
                        *x,
                        *y,
                        *width,
                        *height,
                        state.overlay_color,
                        state.overlay_alpha,
                    ) {
                        Ok(hwnd) => windows.push(hwnd),
                        Err(e) => return Err(format!("Failed to create {} window: {}", name, e)),
                    }
                }
            }
        }
    }

    Ok(windows)
}

fn create_single_overlay_window(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    _color: u32,
    alpha: u8,
) -> Result<HWND, String> {
    unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let class_name: Vec<u16> = "MouseBarrierOverlay\0".encode_utf16().collect();

        // Check if class is already registered
        let mut wc_existing: WNDCLASSEXW = mem::zeroed();
        wc_existing.cbSize = mem::size_of::<WNDCLASSEXW>() as u32;

        if GetClassInfoExW(instance, class_name.as_ptr(), &mut wc_existing) == 0 {
            // Class not registered, so register it
            let wc = WNDCLASSEXW {
                cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: ptr::null_mut(),
                hCursor: ptr::null_mut(),
                hbrBackground: ptr::null_mut(), // No background brush
                lpszMenuName: ptr::null(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: ptr::null_mut(),
            };

            if RegisterClassExW(&wc) == 0 {
                return Err(format!(
                    "Failed to register window class: {}",
                    GetLastError()
                ));
            }
        }

        // Use the provided window dimensions

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            class_name.as_ptr(),
            WS_POPUP,
            x,
            y,
            width,
            height,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        );

        if hwnd.is_null() {
            return Err(format!("Failed to create window: {}", GetLastError()));
        }

        // Use configurable alpha transparency
        SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA);

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

        Ok(hwnd)
    }
}
