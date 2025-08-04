use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tracing::{info, warn};
use winapi::shared::minwindef::{HMODULE, LPARAM, LRESULT, TRUE, UINT, WPARAM};
use winapi::shared::windef::{HWND, POINT, RECT};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::libloaderapi::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

type KeyboardCallback = Arc<Mutex<Option<Box<dyn Fn(u32, bool) + Send + Sync>>>>;
type MousePositionCallback = Arc<Mutex<Option<Box<dyn Fn(i32, i32) + Send + Sync>>>>;

static MOUSE_BARRIER_STATE: OnceLock<Arc<Mutex<Option<MouseBarrierState>>>> = OnceLock::new();
static KEYBOARD_CALLBACK: OnceLock<KeyboardCallback> = OnceLock::new();
static MOUSE_POSITION_CALLBACK: OnceLock<MousePositionCallback> = OnceLock::new();
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
static HAS_ENTERED_BARRIER: AtomicBool = AtomicBool::new(false);
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
    on_barrier_hit_sound: Option<String>,
    on_barrier_entry_sound: Option<String>,
}

pub struct MouseBarrierConfig {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub buffer_zone: i32,
    pub push_factor: i32,
    pub overlay_color: (u8, u8, u8),
    pub overlay_alpha: u8,
    pub on_barrier_hit_sound: Option<String>,
    pub on_barrier_entry_sound: Option<String>,
}

pub struct MouseBarrier;

pub struct KeyboardHook;

impl MouseBarrier {
    pub fn new(config: MouseBarrierConfig) -> Self {
        // Convert from bottom-left origin to Windows top-left origin
        let barrier_rect = RECT {
            left: config.x,
            top: config.y - config.height, // y is bottom, so top = y - height
            right: config.x + config.width, // right extends from left
            bottom: config.y,              // bottom is the y coordinate
        };

        let state = MouseBarrierState {
            barrier_rect,
            buffer_zone: config.buffer_zone,
            push_factor: config.push_factor,
            enabled: false,
            overlay_color: ((config.overlay_color.0 as u32) << 16)
                | ((config.overlay_color.1 as u32) << 8)
                | (config.overlay_color.2 as u32),
            overlay_alpha: config.overlay_alpha,
            on_barrier_hit_sound: config.on_barrier_hit_sound,
            on_barrier_entry_sound: config.on_barrier_entry_sound,
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

    pub fn update_barrier(&mut self, config: MouseBarrierConfig) {
        let state_lock = MOUSE_BARRIER_STATE.get().unwrap();
        if let Some(ref mut state) = *state_lock.lock().unwrap() {
            // Convert from bottom-left origin to Windows top-left origin
            state.barrier_rect = RECT {
                left: config.x,
                top: config.y - config.height, // y is bottom, so top = y - height
                right: config.x + config.width, // right extends from left
                bottom: config.y,              // bottom is the y coordinate
            };
            state.buffer_zone = config.buffer_zone;
            state.push_factor = config.push_factor;
            state.overlay_color = ((config.overlay_color.0 as u32) << 16)
                | ((config.overlay_color.1 as u32) << 8)
                | (config.overlay_color.2 as u32);
            state.overlay_alpha = config.overlay_alpha;
            state.on_barrier_hit_sound = config.on_barrier_hit_sound;
            state.on_barrier_entry_sound = config.on_barrier_entry_sound;

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
                            let last = *last_pos_guard;
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
                            if let Some(safe_pos) = check_movement_path(
                                &last,
                                &current_pos,
                                &state.barrier_rect,
                                &buffer_rect,
                            ) {
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
                                let push_factor = calculate_dynamic_push_factor(
                                    state.push_factor,
                                    &last,
                                    &current_pos,
                                );
                                let safe_pos =
                                    push_point_out_of_rect(&current_pos, &buffer_rect, push_factor);
                                SetCursorPos(safe_pos.x, safe_pos.y);
                                return 1;
                            }
                        }

                        if point_in_rect(&current_pos, &state.barrier_rect) {
                            warn!(x = current_pos.x, y = current_pos.y, "Cursor in barrier!");

                            // Play barrier entry sound if this is the first time
                            if !HAS_ENTERED_BARRIER.load(Ordering::Acquire) {
                                HAS_ENTERED_BARRIER.store(true, Ordering::Release);
                                if let Some(ref sound_path) = state.on_barrier_entry_sound {
                                    play_sound_async(sound_path);
                                }
                            }
                        } else {
                            // Reset the flag when cursor leaves barrier
                            HAS_ENTERED_BARRIER.store(false, Ordering::Release);
                        }

                        let in_buffer = point_in_rect(&current_pos, &buffer_rect);
                        let was_in_buffer = LAST_IN_BARRIER.load(Ordering::Acquire);

                        if in_buffer != was_in_buffer {
                            LAST_IN_BARRIER.store(in_buffer, Ordering::Release);

                            // Play barrier hit sound when entering buffer zone
                            if in_buffer {
                                if let Some(ref sound_path) = state.on_barrier_hit_sound {
                                    play_sound_async(sound_path);
                                }
                            }
                        }

                        if in_buffer {
                            // Calculate dynamic push factor based on movement speed
                            let push_factor = if let Some(last) = last_pos {
                                calculate_dynamic_push_factor(
                                    state.push_factor,
                                    &last,
                                    &current_pos,
                                )
                            } else {
                                state.push_factor
                            };

                            let new_pos =
                                push_point_out_of_rect(&current_pos, &buffer_rect, push_factor);

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

fn play_sound_async(sound_path: &str) {
    let path = sound_path.to_string();
    thread::spawn(move || {
        unsafe {
            // Load winmm.dll dynamically
            let winmm_name: Vec<u16> = "winmm\0".encode_utf16().collect();
            let winmm = LoadLibraryW(winmm_name.as_ptr());
            if winmm.is_null() {
                warn!("Failed to load winmm.dll for audio playback");
                return;
            }

            // Get PlaySoundW function
            let playsound_name = b"PlaySoundW\0";
            let playsound_proc = GetProcAddress(winmm, playsound_name.as_ptr() as *const i8);
            if playsound_proc.is_null() {
                warn!("Failed to find PlaySoundW function");
                return;
            }

            // Cast to function pointer and call
            type PlaySoundWFn = unsafe extern "system" fn(*const u16, HMODULE, u32) -> i32;
            let playsound_fn: PlaySoundWFn = std::mem::transmute(playsound_proc);

            let wide_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            // SND_FILENAME = 0x00020000, SND_ASYNC = 0x0001, SND_NODEFAULT = 0x0002
            playsound_fn(
                wide_path.as_ptr(),
                std::ptr::null_mut(),
                0x00020000 | 0x0001 | 0x0002,
            );
        }
    });
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
    let multiplier = (speed / 25.0).clamp(1.0, 3.0);
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
            let clamped_buffer_bottom = buffer_bottom.min(screen_height);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_barrier_config_creation() {
        let config = MouseBarrierConfig {
            x: 100,
            y: 200,
            width: 300,
            height: 150,
            buffer_zone: 25,
            push_factor: 50,
            overlay_color: (255, 128, 64),
            overlay_alpha: 200,
            on_barrier_hit_sound: Some("hit.wav".to_string()),
            on_barrier_entry_sound: None,
        };

        assert_eq!(config.x, 100);
        assert_eq!(config.y, 200);
        assert_eq!(config.width, 300);
        assert_eq!(config.height, 150);
        assert_eq!(config.buffer_zone, 25);
        assert_eq!(config.push_factor, 50);
        assert_eq!(config.overlay_color, (255, 128, 64));
        assert_eq!(config.overlay_alpha, 200);
        assert_eq!(config.on_barrier_hit_sound, Some("hit.wav".to_string()));
        assert_eq!(config.on_barrier_entry_sound, None);
    }

    #[test]
    fn test_point_in_rect() {
        let rect = RECT {
            left: 10,
            top: 20,
            right: 100,
            bottom: 80,
        };

        // Point inside
        let inside_point = POINT { x: 50, y: 40 };
        assert!(point_in_rect(&inside_point, &rect));

        // Point on boundary (excluded)
        let boundary_point = POINT { x: 100, y: 40 };
        assert!(!point_in_rect(&boundary_point, &rect));

        // Point outside
        let outside_point = POINT { x: 150, y: 40 };
        assert!(!point_in_rect(&outside_point, &rect));

        // Corner cases
        let left_edge = POINT { x: 10, y: 40 };
        assert!(point_in_rect(&left_edge, &rect));

        let top_edge = POINT { x: 50, y: 20 };
        assert!(point_in_rect(&top_edge, &rect));
    }

    #[test]
    fn test_calculate_dynamic_push_factor() {
        let last_pos = POINT { x: 0, y: 0 };
        let base_factor = 50;

        // No movement
        let current_pos = POINT { x: 0, y: 0 };
        let result = calculate_dynamic_push_factor(base_factor, &last_pos, &current_pos);
        assert_eq!(result, base_factor); // Should be 1x multiplier

        // Slow movement (speed < 25)
        let current_pos = POINT { x: 10, y: 0 };
        let result = calculate_dynamic_push_factor(base_factor, &last_pos, &current_pos);
        assert_eq!(result, base_factor); // Should be 1x multiplier

        // Medium movement (speed = 25)
        let current_pos = POINT { x: 25, y: 0 };
        let result = calculate_dynamic_push_factor(base_factor, &last_pos, &current_pos);
        assert_eq!(result, base_factor); // Should be 1x multiplier

        // Fast movement (speed = 50)
        let current_pos = POINT { x: 50, y: 0 };
        let result = calculate_dynamic_push_factor(base_factor, &last_pos, &current_pos);
        assert_eq!(result, 100); // Should be 2x multiplier

        // Very fast movement (speed = 75, should clamp to 3x)
        let current_pos = POINT { x: 75, y: 0 };
        let result = calculate_dynamic_push_factor(base_factor, &last_pos, &current_pos);
        assert_eq!(result, 150); // Should be 3x multiplier

        // Extremely fast movement (should clamp to 3x max)
        let current_pos = POINT { x: 1000, y: 0 };
        let result = calculate_dynamic_push_factor(base_factor, &last_pos, &current_pos);
        assert_eq!(result, 150); // Should be clamped to 3x multiplier
    }

    #[test]
    fn test_push_point_out_of_rect_basic() {
        // Simple test case - mock screen size
        SCREEN_WIDTH.store(1920, Ordering::Relaxed);
        SCREEN_HEIGHT.store(1080, Ordering::Relaxed);

        let rect = RECT {
            left: 100,
            top: 100,
            right: 200,
            bottom: 200,
        };
        let push_factor = 20;

        // Point inside rect - should be pushed out
        let point = POINT { x: 150, y: 150 };
        let pushed = push_point_out_of_rect(&point, &rect, push_factor);

        // The point should be moved outside the rect
        assert!(!point_in_rect(&pushed, &rect));
    }

    #[test]
    fn test_check_movement_path_no_collision() {
        let start = POINT { x: 50, y: 50 };
        let end = POINT { x: 60, y: 50 };
        let barrier = RECT {
            left: 100,
            top: 100,
            right: 200,
            bottom: 200,
        };
        let buffer = RECT {
            left: 90,
            top: 90,
            right: 210,
            bottom: 210,
        };

        let result = check_movement_path(&start, &end, &barrier, &buffer);
        assert!(result.is_none()); // No collision, should return None
    }

    #[test]
    fn test_check_movement_path_small_movement() {
        let start = POINT { x: 50, y: 50 };
        let end = POINT { x: 51, y: 50 }; // Very small movement
        let barrier = RECT {
            left: 100,
            top: 100,
            right: 200,
            bottom: 200,
        };
        let buffer = RECT {
            left: 90,
            top: 90,
            right: 210,
            bottom: 210,
        };

        let result = check_movement_path(&start, &end, &barrier, &buffer);
        assert!(result.is_none()); // Should skip small movements
    }

    #[test]
    fn test_check_movement_path_collision() {
        let start = POINT { x: 50, y: 150 };
        let end = POINT { x: 250, y: 150 }; // Path goes through barrier
        let barrier = RECT {
            left: 100,
            top: 100,
            right: 200,
            bottom: 200,
        };
        let buffer = RECT {
            left: 90,
            top: 90,
            right: 210,
            bottom: 210,
        };

        let result = check_movement_path(&start, &end, &barrier, &buffer);
        assert!(result.is_some()); // Should detect collision and return safe point

        let safe_point = result.unwrap();
        assert!(!point_in_rect(&safe_point, &buffer)); // Safe point should be outside buffer
    }

    #[test]
    fn test_mouse_barrier_state_creation() {
        let state = MouseBarrierState {
            barrier_rect: RECT {
                left: 0,
                top: 0,
                right: 100,
                bottom: 100,
            },
            buffer_zone: 10,
            push_factor: 30,
            enabled: false,
            overlay_color: 0xFF0000,
            overlay_alpha: 128,
            on_barrier_hit_sound: Some("sound.wav".to_string()),
            on_barrier_entry_sound: None,
        };

        assert_eq!(state.buffer_zone, 10);
        assert_eq!(state.push_factor, 30);
        assert!(!state.enabled);
        assert_eq!(state.overlay_color, 0xFF0000);
        assert_eq!(state.overlay_alpha, 128);
        assert_eq!(state.on_barrier_hit_sound, Some("sound.wav".to_string()));
        assert_eq!(state.on_barrier_entry_sound, None);
    }

    // Test helper functions
    #[test]
    fn test_coordinate_conversion_logic() {
        // Test the coordinate conversion from bottom-left to top-left origin
        let x = 100;
        let y = 500; // This is bottom coordinate
        let width = 200;
        let height = 100;

        let expected_rect = RECT {
            left: x,
            top: y - height,  // top = 500 - 100 = 400
            right: x + width, // right = 100 + 200 = 300
            bottom: y,        // bottom = 500
        };

        assert_eq!(expected_rect.left, 100);
        assert_eq!(expected_rect.top, 400);
        assert_eq!(expected_rect.right, 300);
        assert_eq!(expected_rect.bottom, 500);
    }

    #[test]
    fn test_overlay_color_conversion() {
        let r = 255u8;
        let g = 128u8;
        let b = 64u8;

        let expected_color = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        assert_eq!(expected_color, 0xFF8040);

        // Test different color combinations
        let white = ((255u8 as u32) << 16) | ((255u8 as u32) << 8) | (255u8 as u32);
        assert_eq!(white, 0xFFFFFF);

        let black = ((0u8 as u32) << 16) | ((0u8 as u32) << 8) | (0u8 as u32);
        assert_eq!(black, 0x000000);

        let red = ((255u8 as u32) << 16) | ((0u8 as u32) << 8) | (0u8 as u32);
        assert_eq!(red, 0xFF0000);

        let green = ((0u8 as u32) << 16) | ((255u8 as u32) << 8) | (0u8 as u32);
        assert_eq!(green, 0x00FF00);

        let blue = ((0u8 as u32) << 16) | ((0u8 as u32) << 8) | (255u8 as u32);
        assert_eq!(blue, 0x0000FF);
    }
}
