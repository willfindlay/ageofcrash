use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicPtr, Ordering};
use tracing::{debug, info, warn};
use winapi::um::winuser::*;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::shared::windef::{POINT, RECT};
use winapi::shared::minwindef::{WPARAM, LPARAM, LRESULT};
use winapi::um::errhandlingapi::GetLastError;

static MOUSE_BARRIER_STATE: OnceLock<Arc<Mutex<Option<MouseBarrierState>>>> = OnceLock::new();
static KEYBOARD_CALLBACK: OnceLock<Arc<Mutex<Option<Box<dyn Fn(u32, bool) + Send + Sync>>>>> = OnceLock::new();
static KEYBOARD_HOOK_HANDLE: AtomicPtr<winapi::shared::windef::HHOOK__> = AtomicPtr::new(std::ptr::null_mut());
static MOUSE_HOOK_HANDLE: AtomicPtr<winapi::shared::windef::HHOOK__> = AtomicPtr::new(std::ptr::null_mut());
static LAST_IN_BARRIER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Clone)]
struct MouseBarrierState {
    barrier_rect: RECT,
    push_factor: i32,
    enabled: bool,
}

pub struct MouseBarrier;

pub struct KeyboardHook;

impl MouseBarrier {
    pub fn new(x: i32, y: i32, width: i32, height: i32, push_factor: i32) -> Self {
        // Convert from bottom-left origin to Windows top-left origin
        let barrier_rect = RECT {
            left: x,
            top: y - height,    // y is bottom, so top = y - height
            right: x + width,   // right extends from left
            bottom: y,          // bottom is the y coordinate
        };

        let state = MouseBarrierState {
            barrier_rect,
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

    pub fn update_barrier(&mut self, x: i32, y: i32, width: i32, height: i32, push_factor: i32) {
        let state_lock = MOUSE_BARRIER_STATE.get().unwrap();
        if let Some(ref mut state) = *state_lock.lock().unwrap() {
            // Convert from bottom-left origin to Windows top-left origin
            state.barrier_rect = RECT {
                left: x,
                top: y - height,    // y is bottom, so top = y - height
                right: x + width,   // right extends from left
                bottom: y,          // bottom is the y coordinate
            };
            state.push_factor = push_factor;
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
                        
                        let in_barrier = point_in_rect(&current_pos, &state.barrier_rect);
                        let was_in_barrier = LAST_IN_BARRIER.load(Ordering::Acquire);
                        
                        if in_barrier && !was_in_barrier {
                            debug!(x = current_pos.x, y = current_pos.y, "Mouse entered barrier zone");
                            LAST_IN_BARRIER.store(true, Ordering::Release);
                        } else if !in_barrier && was_in_barrier {
                            debug!("Mouse exited barrier zone");
                            LAST_IN_BARRIER.store(false, Ordering::Release);
                        }
                        
                        if in_barrier {
                            let new_pos = push_point_out_of_rect(&current_pos, &state.barrier_rect, state.push_factor);
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