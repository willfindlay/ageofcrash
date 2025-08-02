use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicPtr, Ordering};
use tracing::{debug, info, warn};
use winapi::um::winuser::*;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::shared::windef::{POINT, RECT, HWND};
use winapi::shared::minwindef::{WPARAM, LPARAM, LRESULT, UINT, TRUE};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::wingdi::*;
use winapi::shared::windef::{SIZE};
use std::ptr;
use std::mem;

static MOUSE_BARRIER_STATE: OnceLock<Arc<Mutex<Option<MouseBarrierState>>>> = OnceLock::new();
static KEYBOARD_CALLBACK: OnceLock<Arc<Mutex<Option<Box<dyn Fn(u32, bool) + Send + Sync>>>>> = OnceLock::new();
static KEYBOARD_HOOK_HANDLE: AtomicPtr<winapi::shared::windef::HHOOK__> = AtomicPtr::new(std::ptr::null_mut());
static MOUSE_HOOK_HANDLE: AtomicPtr<winapi::shared::windef::HHOOK__> = AtomicPtr::new(std::ptr::null_mut());
static LAST_IN_BARRIER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static OVERLAY_WINDOWS: [AtomicPtr<winapi::shared::windef::HWND__>; 4] = [
    AtomicPtr::new(std::ptr::null_mut()),
    AtomicPtr::new(std::ptr::null_mut()),
    AtomicPtr::new(std::ptr::null_mut()),
    AtomicPtr::new(std::ptr::null_mut()),
];

#[derive(Clone)]
struct MouseBarrierState {
    barrier_rect: RECT,
    buffer_zone: i32,
    push_factor: i32,
    enabled: bool,
}

pub struct MouseBarrier;

pub struct KeyboardHook;

impl MouseBarrier {
    pub fn new(x: i32, y: i32, width: i32, height: i32, buffer_zone: i32, push_factor: i32) -> Self {
        // Convert from bottom-left origin to Windows top-left origin
        let barrier_rect = RECT {
            left: x,
            top: y - height,    // y is bottom, so top = y - height
            right: x + width,   // right extends from left
            bottom: y,          // bottom is the y coordinate
        };

        let state = MouseBarrierState {
            barrier_rect,
            buffer_zone,
            push_factor,
            enabled: false,
        };

        let state_lock = MOUSE_BARRIER_STATE.get_or_init(|| Arc::new(Mutex::new(None)));
        *state_lock.lock().unwrap() = Some(state);

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

    pub fn disable(&mut self) -> Result<(), String> {
        let hook = MOUSE_HOOK_HANDLE.swap(std::ptr::null_mut(), Ordering::AcqRel);
        
        if !hook.is_null() {
            let state_lock = MOUSE_BARRIER_STATE.get().unwrap();
            if let Some(ref mut state) = *state_lock.lock().unwrap() {
                state.enabled = false;
            }
            
            unsafe {
                if UnhookWindowsHookEx(hook) == 0 {
                    return Err(format!("Failed to unhook mouse: {}", GetLastError()));
                }
            }
        }
        
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

    pub fn update_barrier(&mut self, x: i32, y: i32, width: i32, height: i32, buffer_zone: i32, push_factor: i32) {
        let state_lock = MOUSE_BARRIER_STATE.get().unwrap();
        if let Some(ref mut state) = *state_lock.lock().unwrap() {
            // Convert from bottom-left origin to Windows top-left origin
            state.barrier_rect = RECT {
                left: x,
                top: y - height,    // y is bottom, so top = y - height
                right: x + width,   // right extends from left
                bottom: y,          // bottom is the y coordinate
            };
            state.buffer_zone = buffer_zone;
            state.push_factor = push_factor;
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

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && wparam == WM_MOUSEMOVE as WPARAM {
        if let Some(state_lock) = MOUSE_BARRIER_STATE.get() {
            if let Ok(state_guard) = state_lock.lock() {
                if let Some(ref state) = *state_guard {
                    if state.enabled {
                        let mouse_data = *(lparam as *const MSLLHOOKSTRUCT);
                        let current_pos = mouse_data.pt;
                        
                        // Create buffer zone rect
                        let buffer_rect = RECT {
                            left: state.barrier_rect.left - state.buffer_zone,
                            top: state.barrier_rect.top - state.buffer_zone,
                            right: state.barrier_rect.right + state.buffer_zone,
                            bottom: state.barrier_rect.bottom + state.buffer_zone,
                        };
                        
                        let in_buffer = point_in_rect(&current_pos, &buffer_rect);
                        let was_in_buffer = LAST_IN_BARRIER.load(Ordering::Acquire);
                        
                        if in_buffer && !was_in_buffer {
                            debug!(x = current_pos.x, y = current_pos.y, "Mouse entered buffer zone");
                            LAST_IN_BARRIER.store(true, Ordering::Release);
                        } else if !in_buffer && was_in_buffer {
                            debug!("Mouse exited buffer zone");
                            LAST_IN_BARRIER.store(false, Ordering::Release);
                        }
                        
                        if in_buffer {
                            let new_pos = push_point_out_of_rect(&current_pos, &buffer_rect, state.push_factor);
                            debug!(
                                from.x = current_pos.x, from.y = current_pos.y,
                                to.x = new_pos.x, to.y = new_pos.y,
                                "Mouse position pushed"
                            );
                            
                            let result = SetCursorPos(new_pos.x, new_pos.y);
                            if result == 0 {
                                warn!(error = GetLastError(), "SetCursorPos failed");
                            } else {
                                debug!("SetCursorPos succeeded");
                            }
                            
                            // Verify where the cursor actually ended up
                            let mut actual_pos = POINT { x: 0, y: 0 };
                            if GetCursorPos(&mut actual_pos) != 0 {
                                debug!(
                                    actual.x = actual_pos.x, actual.y = actual_pos.y,
                                    "Cursor actual position after push"
                                );
                            }
                            
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
                    let is_key_down = wparam == WM_KEYDOWN as WPARAM || wparam == WM_SYSKEYDOWN as WPARAM;
                    callback(kbd_data.vkCode, is_key_down);
                }
            }
        }
    }

    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

fn point_in_rect(point: &POINT, rect: &RECT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

fn push_point_out_of_rect(point: &POINT, rect: &RECT, push_factor: i32) -> POINT {
    debug!(
        mouse.x = point.x, mouse.y = point.y,
        rect.left = rect.left, rect.top = rect.top, rect.right = rect.right, rect.bottom = rect.bottom,
        "Push logic - analyzing mouse position in barrier"
    );
    
    // Debug: Check what Windows thinks the screen size is
    let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    debug!(width = screen_width, height = screen_height, "Screen dimensions from GetSystemMetrics");
    
    let center_x = (rect.left + rect.right) / 2;
    let center_y = (rect.top + rect.bottom) / 2;
    
    let dx = point.x - center_x;
    let dy = point.y - center_y;
    
    debug!(
        center.x = center_x, center.y = center_y,
        direction.dx = dx, direction.dy = dy,
        "Barrier center and direction calculation"
    );
    
    // Determine which edge the mouse is closest to and push away from that edge
    let dist_to_left = point.x - rect.left;
    let dist_to_right = rect.right - point.x;
    let dist_to_top = point.y - rect.top;
    let dist_to_bottom = rect.bottom - point.y;
    
    debug!(
        distances.left = dist_to_left, distances.right = dist_to_right,
        distances.top = dist_to_top, distances.bottom = dist_to_bottom,
        "Distance to each barrier edge"
    );
    
    // Find the minimum distance to determine which edge to push from
    let min_dist = dist_to_left.min(dist_to_right).min(dist_to_top).min(dist_to_bottom);
    
    let (new_point, direction) = if min_dist == dist_to_left {
        (POINT { x: rect.left - push_factor, y: point.y }, "LEFT")
    } else if min_dist == dist_to_right {
        (POINT { x: rect.right + push_factor, y: point.y }, "RIGHT")
    } else if min_dist == dist_to_top {
        (POINT { x: point.x, y: rect.top - push_factor }, "UP")
    } else {
        (POINT { x: point.x, y: rect.bottom + push_factor }, "DOWN")
    };
    
    debug!(
        direction = direction,
        physical.x = new_point.x, physical.y = new_point.y,
        will_clamp = new_point.y > screen_height,
        "Push direction determined"
    );
    
    // Convert from physical coordinates to logical coordinates for SetCursorPos
    // Physical screen: ~3840x2160, Logical screen: 3072x1728
    let scale_x = screen_width as f64 / 3840.0;
    let scale_y = screen_height as f64 / 2160.0;
    
    let logical_x = (new_point.x as f64 * scale_x).round() as i32;  
    let logical_y = (new_point.y as f64 * scale_y).round() as i32;
    
    // Clamp to screen boundaries
    let logical_x = logical_x.max(0).min(screen_width - 1);
    let logical_y = logical_y.max(0).min(screen_height - 1);
    
    let logical_point = POINT { x: logical_x, y: logical_y };
    debug!(
        logical.x = logical_point.x, logical.y = logical_point.y,
        scale.x = scale_x, scale.y = scale_y,
        "Converted to logical coordinates"
    );
    
    logical_point
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            
            // Just draw a simple red rectangle to test if anything shows up
            let red_brush = CreateSolidBrush(RGB(255, 0, 0));
            let mut client_rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetClientRect(hwnd, &mut client_rect);
            FillRect(hdc, &client_rect, red_brush);
            DeleteObject(red_brush as *mut _);
            
            EndPaint(hwnd, &ps);
            0
        }
        WM_ERASEBKGND => {
            1 // Return non-zero to indicate we handled it
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam)
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
                ("top", clamped_buffer_left, clamped_buffer_top, clamped_buffer_right - clamped_buffer_left, barrier_top - clamped_buffer_top),
                ("bottom", clamped_buffer_left, barrier_bottom, clamped_buffer_right - clamped_buffer_left, clamped_buffer_bottom - barrier_bottom),
                ("left", clamped_buffer_left, barrier_top, barrier_left - clamped_buffer_left, barrier_bottom - barrier_top),
                ("right", barrier_right, barrier_top, clamped_buffer_right - barrier_right, barrier_bottom - barrier_top),
            ];
            
            for (name, x, y, width, height) in window_configs.iter() {
                if *width > 0 && *height > 0 {
                    match create_single_overlay_window(*x, *y, *width, *height) {
                        Ok(hwnd) => windows.push(hwnd),
                        Err(e) => return Err(format!("Failed to create {} window: {}", name, e)),
                    }
                }
            }
        }
    }
    
    Ok(windows)
}

fn create_single_overlay_window(x: i32, y: i32, width: i32, height: i32) -> Result<HWND, String> {
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
                return Err(format!("Failed to register window class: {}", GetLastError()));
            }
        }
        
        // Use the provided window dimensions
        
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            class_name.as_ptr(),
            WS_POPUP,
            x, y, width, height,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        );
        
        if hwnd.is_null() {
            return Err(format!("Failed to create window: {}", GetLastError()));
        }
        
        // Use simple alpha transparency - make it very visible for debugging
        SetLayeredWindowAttributes(hwnd, 0, 230, LWA_ALPHA);
        
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        
        Ok(hwnd)
    }
}