#![windows_subsystem = "windows"]

use minifb::{Key, Window, WindowOptions};
use glam::{Vec3, Vec4, Mat4};

#[cfg(windows)]
use winapi::um::winuser::{
    SetWindowLongPtrW, GetWindowLongPtrW, GWL_STYLE,
    WS_BORDER, WS_CAPTION, WS_THICKFRAME, WS_SYSMENU, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
    SetWindowPos, HWND_TOPMOST, HWND_NOTOPMOST,
    SWP_FRAMECHANGED, SWP_SHOWWINDOW, SWP_ASYNCWINDOWPOS,
    GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
};
#[cfg(windows)]
use winapi::shared::windef::HWND;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;
const NUM_STARS: usize = 1000;
const PI: f32 = std::f32::consts::PI;

struct Starfield {
    x:     Vec<f32>,
    y:     Vec<f32>,
    z:     Vec<f32>,
    vel:   Vec<f32>,
    dirty: Vec<usize>,
}

impl Starfield {
    fn new(count: usize) -> Self {
        let mut sf = Starfield {
            x:     Vec::with_capacity(count),
            y:     Vec::with_capacity(count),
            z:     Vec::with_capacity(count),
            vel:   Vec::with_capacity(count),
            dirty: Vec::with_capacity(count),
        };
        for _ in 0..count {
            sf.x.push((fastrand::f32() - 0.5) * 2.0);
            sf.y.push((fastrand::f32() - 0.5) * 2.0);
            sf.z.push(fastrand::f32());
            sf.vel.push(0.01 + fastrand::f32() * 0.05);
        }
        sf
    }

    #[inline(always)]
    fn reset_star(x: &mut f32, y: &mut f32, z: &mut f32) {
        *x = (fastrand::f32() - 0.5) * 2.0;
        *y = (fastrand::f32() - 0.5) * 2.0;
        *z = 1.0;
    }
}

#[inline(always)]
fn extract_vp_rows(vp: &Mat4) -> (Vec4, Vec4, Vec4, Vec4) {
    let t = vp.transpose();
    (t.x_axis, t.y_axis, t.z_axis, t.w_axis)
}

fn main() {
    let mut sf = Starfield::new(NUM_STARS);
    let mut buffer: Vec<u32> = vec![0u32; WIDTH * HEIGHT];

    let mut is_fullscreen = false;
    let mut win_w = WIDTH;
    let mut win_h = HEIGHT;

    let mut window = Window::new(
        "Starfield",
        win_w,
        win_h,
        WindowOptions {
            scale: minifb::Scale::X1,
            borderless: true,
            ..WindowOptions::default()
        },
    )
    .unwrap();

    #[cfg(windows)]
    let hwnd: HWND = window.get_window_handle() as HWND;

    #[cfg(windows)]
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let stripped = style
            & !(WS_BORDER | WS_CAPTION | WS_THICKFRAME | WS_SYSMENU
                | WS_MAXIMIZEBOX | WS_MINIMIZEBOX);
        SetWindowLongPtrW(hwnd, GWL_STYLE, stripped as isize);
        let cx = GetSystemMetrics(SM_CXSCREEN);
        let cy = GetSystemMetrics(SM_CYSCREEN);
        let x = (cx - WIDTH as i32) / 2;
        let y = (cy - HEIGHT as i32) / 2;
        SetWindowPos(hwnd, HWND_NOTOPMOST, x, y, WIDTH as i32, HEIGHT as i32,
                     SWP_FRAMECHANGED | SWP_SHOWWINDOW);
    }

    window.limit_update_rate(Some(std::time::Duration::from_micros(16_600)));

    let mut cam_az:  f32 = 0.0;
    let mut cam_el:  f32 = 0.5;
    const CAM_R:     f32 = 3.5;
    let mut dragging      = false;
    let mut last_mx: f32 = 0.0;
    let mut last_my: f32 = 0.0;
    let mut was_f11      = false;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let f11 = window.is_key_down(Key::F11);
        if f11 && !was_f11 {
            is_fullscreen = !is_fullscreen;
            #[cfg(windows)]
            unsafe {
                if is_fullscreen {
                    let sw = GetSystemMetrics(SM_CXSCREEN);
                    let sh = GetSystemMetrics(SM_CYSCREEN);
                    win_w = sw as usize;
                    win_h = sh as usize;
                    SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, sw, sh,
                                 SWP_SHOWWINDOW | SWP_ASYNCWINDOWPOS);
                } else {
                    win_w = WIDTH;
                    win_h = HEIGHT;
                    let x = (GetSystemMetrics(SM_CXSCREEN) - WIDTH as i32) / 2;
                    let y = (GetSystemMetrics(SM_CYSCREEN) - HEIGHT as i32) / 2;
                    SetWindowPos(hwnd, HWND_NOTOPMOST, x, y,
                                 WIDTH as i32, HEIGHT as i32,
                                 SWP_SHOWWINDOW | SWP_ASYNCWINDOWPOS);
                }
            }
            buffer.resize(win_w * win_h, 0);
            buffer.fill(0);
            sf.dirty.clear();
        }
        was_f11 = f11;

        if let Some((mx, my)) = window.get_mouse_pos(minifb::MouseMode::Clamp) {
            let mx = mx as f32;
            let my = my as f32;
            if window.get_mouse_down(minifb::MouseButton::Left) {
                if dragging {
                    cam_az -= (mx - last_mx) * 0.01;
                    cam_el  = (cam_el + (my - last_my) * 0.01)
                        .clamp(-PI / 2.0 + 0.1, PI / 2.0 - 0.1);
                }
                dragging = true;
                last_mx = mx;
                last_my = my;
            } else {
                dragging = false;
            }
        }

        for &idx in &sf.dirty {
            if idx < buffer.len() {
                buffer[idx] = 0;
            }
        }
        sf.dirty.clear();

        let cam_pos = Vec3::new(
            CAM_R * cam_el.cos() * cam_az.sin(),
            CAM_R * cam_el.sin(),
            CAM_R * cam_el.cos() * cam_az.cos(),
        );
        let proj = Mat4::perspective_rh_gl(PI / 3.0, win_w as f32 / win_h as f32, 0.1, 100.0);
        let view = Mat4::look_at_rh(cam_pos, Vec3::ZERO, Vec3::Y);
        let vp   = proj * view;
        let (row0, row1, row2, row3) = extract_vp_rows(&vp);

        let hw = win_w as f32;
        let hh = win_h as f32;
        let wi = win_w as i32;
        let hi = win_h as i32;

        for i in 0..NUM_STARS {
            sf.z[i] -= sf.vel[i];
            if sf.z[i] < 0.0 {
                Starfield::reset_star(&mut sf.x[i], &mut sf.y[i], &mut sf.z[i]);
            }

            let p = Vec4::new(sf.x[i], sf.y[i], sf.z[i], 1.0);
            let cx_ = row0.dot(p);
            let cy_ = row1.dot(p);
            let cz_ = row2.dot(p);
            let cw_ = row3.dot(p);

            if cz_ < -cw_ || cz_ > cw_ || cw_ <= 0.0 {
                continue;
            }

            let inv_w = 1.0 / cw_;
            let sx = ((cx_ * inv_w * 0.5 + 0.5) * hw) as i32;
            let sy = ((0.5 - cy_ * inv_w * 0.5) * hh) as i32;

            if sx < 0 || sx >= wi || sy < 0 || sy >= hi {
                continue;
            }

            let idx = sy as usize * win_w + sx as usize;
            let b   = (255.0 * (1.0 - sf.z[i])) as u32;
            buffer[idx] = (b << 16) | (b << 8) | b;
            sf.dirty.push(idx);
        }

        window.update_with_buffer(&buffer, win_w, win_h).unwrap();
    }
}
