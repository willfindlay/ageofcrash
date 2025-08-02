use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicPtr, Ordering};
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
                            println!("Mouse entered barrier zone at ({}, {})", current_pos.x, current_pos.y);
                            LAST_IN_BARRIER.store(true, Ordering::Release);
                        } else if !in_barrier && was_in_barrier {
                            println!("Mouse exited barrier zone");
                            LAST_IN_BARRIER.store(false, Ordering::Release);
                        }
                        
                        if in_barrier {
                            let new_pos = push_point_out_of_rect(&current_pos, &state.barrier_rect, state.push_factor);
                            println!("Mouse pushed from ({}, {}) to ({}, {})", 
                                current_pos.x, current_pos.y, new_pos.x, new_pos.y);
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
    let center_x = (rect.left + rect.right) / 2;
    let center_y = (rect.top + rect.bottom) / 2;
    
    let dx = point.x - center_x;
    let dy = point.y - center_y;
    
    let new_x = if dx.abs() > dy.abs() {
        if dx > 0 {
            rect.right + push_factor
        } else {
            rect.left - push_factor
        }
    } else {
        point.x
    };
    
    let new_y = if dy.abs() > dx.abs() {
        if dy > 0 {
            rect.bottom + push_factor
        } else {
            rect.top - push_factor
        }
    } else {
        point.y
    };
    
    POINT { x: new_x, y: new_y }
}