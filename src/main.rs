#![windows_subsystem = "windows"]

use ab_glyph::{FontRef, PxScale};
use image::{Rgb, RgbImage, RgbaImage};
use imageproc::drawing::{draw_text_mut, text_size};
use rand::{rngs::StdRng, Rng, SeedableRng};
use rayon::prelude::*;
use std::{
    env,
    ffi::OsStr,
    fs,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, TrayIconBuilder,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_SETDESKWALLPAPER,
};
use winit::event_loop::{ControlFlow, EventLoop};
use winrt_notification::{Duration as ToastDuration, IconCrop, Sound, Toast};

const W: u32 = 2560;
const H: u32 = 1440;

const PROGI: [f32; 11] = [0.0, 0.10, 0.22, 0.30, 0.38, 0.50, 0.62, 0.70, 0.78, 0.90, 1.0];
const KOLORY: [(f32, f32, f32); 11] = [
    (20.0, 105.0, 225.0),
    (35.0, 125.0, 235.0),
    (138.0, 65.0, 168.0),
    (212.0, 168.0, 78.0),
    (185.0, 136.0, 64.0),
    (165.0, 170.0, 178.0),
    (185.0, 136.0, 64.0),
    (212.0, 168.0, 78.0),
    (138.0, 65.0, 168.0),
    (35.0, 125.0, 235.0),
    (20.0, 105.0, 225.0),
];

// Easter Egg: Pattern #387 (Czysty, jednolity jasny błękit w całym środku + subtelne akcenty srebra, złota i fioletu)
const PROGI_387: [f32; 11] = [0.0, 0.03, 0.08, 0.16, 0.36, 0.52, 0.70, 0.84, 0.90, 0.96, 1.0];
const KOLORY_387: [(f32, f32, f32); 11] = [
    (165.0, 170.0, 178.0), // 0.00 - Śladowy srebrny rdzeń
    (212.0, 168.0, 78.0),  // 0.03 - Cienka obwódka złota
    (138.0, 62.0, 198.0),  // 0.08 - Cienkie przejście fioletu
    (48.0, 148.0, 250.0),  // 0.16 - Jasny błękit (jednolita baza)
    (48.0, 148.0, 250.0),  // 0.36 - Jasny błękit (zero ciemnych linii)
    (52.0, 154.0, 252.0),  // 0.52 - Jasny błękit (delikatne rozjaśnienie środka)
    (48.0, 148.0, 250.0),  // 0.70 - Jasny błękit (zero ciemnych linii)
    (48.0, 148.0, 250.0),  // 0.84 - Jasny błękit (jednolita baza)
    (138.0, 62.0, 198.0),  // 0.90 - Delikatna fioletowa smuga
    (125.0, 45.0, 185.0),  // 0.96 - Fioletowy szczyt
    (118.0, 38.0, 178.0),  // 1.00 - Najwyższy punkt fioletu
];

fn interp_color(t: f32, is_387: bool) -> (f32, f32, f32) {
    let progi = if is_387 { &PROGI_387 } else { &PROGI };
    let kolory = if is_387 { &KOLORY_387 } else { &KOLORY };

    for i in 0..(progi.len() - 1) {
        let p0 = progi[i];
        let p1 = progi[i + 1];
        if t >= p0 && t <= p1 {
            let ratio = (t - p0) / (p1 - p0);
            let (r0, g0, b0) = kolory[i];
            let (r1, g1, b1) = kolory[i + 1];
            return (
                r0 + ratio * (r1 - r0),
                g0 + ratio * (g1 - g0),
                b0 + ratio * (b1 - b0),
            );
        }
    }
    *kolory.last().unwrap()
}

fn prepare_icons(ico_path: &Path) -> Icon {
    let size = 64u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 * 0.07;
            let fy = y as f32 * 0.07;

            let val = (fx.sin() + fy.cos() + (fx - fy).sin() * 0.6 + 2.6) / 5.2;

            let levels = 5.0;
            let scaled = val * levels;
            let dist = (scaled - scaled.round()).abs();

            let alpha = ((0.14 - dist) / 0.08).clamp(0.0, 1.0);

            rgba.push(255);
            rgba.push(255);
            rgba.push(255);
            rgba.push((alpha * 255.0) as u8);
        }
    }

    if let Some(img_buf) = RgbaImage::from_raw(size, size, rgba.clone()) {
        let _ = img_buf.save(ico_path);
    }

    Icon::from_rgba(rgba, size, size).expect("Nie udało się utworzyć ikonki")
}

fn show_blue_gem_toast(ico_path: &Path) {
    let mut toast = Toast::new(Toast::POWERSHELL_APP_ID)
        .title("Blue Gem #387")
        .text1("Wylosowano legendarny pattern Case Hardened #387! Sprawdź pulpit.")
        .sound(Some(Sound::Reminder))
        .duration(ToastDuration::Long);

    if ico_path.exists() {
        toast = toast.icon(ico_path, IconCrop::Square, "Blue Gem #387");
    }

    let _ = toast.show();
}

fn generate_wallpaper(output_path: &Path, ico_path: &Path) {
    let mut base_rng = rand::thread_rng();
    let seed: u64 = base_rng.gen_range(1..1000);
    let is_387 = seed == 387;
    let mut rng = StdRng::seed_from_u64(seed);

    let two_pi = 2.0 * std::f32::consts::PI;
    let f1 = rng.gen_range(0.0..two_pi);
    let f2 = rng.gen_range(0.0..two_pi);
    let f3 = rng.gen_range(0.0..two_pi);
    let f4 = rng.gen_range(0.0..two_pi);
    let f5 = rng.gen_range(0.0..two_pi);
    let f6 = rng.gen_range(0.0..two_pi);
    let f7 = rng.gen_range(0.0..two_pi);
    let f8 = rng.gen_range(0.0..two_pi);
    let f9 = rng.gen_range(0.0..two_pi);
    let f10 = rng.gen_range(0.0..two_pi);

    let raw_fala: Vec<f32> = (0..(W * H))
        .into_par_iter()
        .map(|idx| {
            let x = (idx % W) as f32;
            let y = (idx / W) as f32;

            let wx = x + 90.0 * (y * 0.006 + f1).sin()
                + 45.0 * (x * 0.011 + f2).cos()
                + 20.0 * (y * 0.022 + f9).sin();

            let wy = y + 90.0 * (x * 0.006 + f3).cos()
                + 45.0 * (y * 0.011 + f4).sin()
                + 20.0 * (x * 0.022 + f10).cos();

            (wx * 0.0095 + f5).sin() * 1.4
                + (wy * 0.010 + f6).cos() * 1.4
                + ((wx - wy) * 0.016 + f7).sin() * 1.0
                + ((wx + wy) * 0.024 + f8).cos() * 0.7
        })
        .collect();

    let (min_val, max_val) = raw_fala
        .par_iter()
        .fold(
            || (f32::MAX, f32::MIN),
            |(min_a, max_a), &v| (min_a.min(v), max_a.max(v)),
        )
        .reduce(
            || (f32::MAX, f32::MIN),
            |(min_a, max_a), (min_b, max_b)| (min_a.min(min_b), max_a.max(max_b)),
        );

    let span = max_val - min_val;

    let mut buffer = vec![0u8; (W * H * 3) as usize];
    buffer
        .par_chunks_mut((W * 3) as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let mut row_rng = StdRng::seed_from_u64(seed.wrapping_add(y as u64 * 7919));
            for (x, pixel) in row.chunks_exact_mut(3).enumerate() {
                let val = raw_fala[y * (W as usize) + x];
                let t = ((val - min_val) / span).clamp(0.0, 1.0);
                let (r, g, b) = interp_color(t, is_387);

                let u_sum: f32 = (0..6).map(|_| row_rng.gen::<f32>()).sum::<f32>() - 3.0;
                let grain = u_sum * 5.5;

                let rgb_bytes = [
                    (r + grain).clamp(0.0, 255.0) as u8,
                    (g + grain).clamp(0.0, 255.0) as u8,
                    (b + grain).clamp(0.0, 255.0) as u8,
                ];
                pixel.copy_from_slice(&rgb_bytes);
            }
        });

    let mut img = RgbImage::from_raw(W, H, buffer).expect("Błąd bufora obrazu");

    if let Ok(font_data) = fs::read("C:\\Windows\\Fonts\\arial.ttf") {
        if let Ok(font) = FontRef::try_from_slice(&font_data) {
            let tekst = if is_387 {
                format!("pattern index: {} [BLUE GEM]", seed)
            } else {
                format!("pattern index: {}", seed)
            };
            let scale = PxScale::from(24.0);
            let (tw, _th) = text_size(scale, &font, &tekst);

            let pos_x = (W - 24).saturating_sub(tw) as i32;
            let pos_y = 24;

            let czarny = Rgb::from([0u8, 0u8, 0u8]);
            draw_text_mut(&mut img, czarny, pos_x, pos_y, scale, &font, &tekst);
        }
    }

    if img.save(output_path).is_ok() {
        set_windows_wallpaper(output_path);
        if is_387 {
            show_blue_gem_toast(ico_path);
        }
    }
}

fn set_windows_wallpaper(path: &Path) {
    let wide_path: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            wide_path.as_ptr() as *mut _,
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        );
    }
}

fn main() {
    let base_dir = env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("."))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let output_path = base_dir.join("case_hardened.png");
    let ico_path = base_dir.join("icon.ico");

    let tray_icon_img = prepare_icons(&ico_path);

    let (tx, rx) = mpsc::channel::<()>();

    thread::spawn(move || loop {
        generate_wallpaper(&output_path, &ico_path);
        let _ = rx.recv_timeout(Duration::from_secs(15 * 60));
    });

    let tray_menu = Menu::new();
    let item_next = MenuItem::new("Losuj nowy pattern teraz", true, None);
    let item_quit = MenuItem::new("Zamknij", true, None);
    tray_menu.append(&item_next).unwrap();
    tray_menu.append(&item_quit).unwrap();

    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("Case Hardened OLED Wallpaper")
        .with_icon(tray_icon_img)
        .build()
        .unwrap();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);

    let next_id = item_next.id().clone();
    let quit_id = item_quit.id().clone();

    #[allow(deprecated)]
    event_loop
        .run(move |_event, elwt| {
            while let Ok(menu_event) = MenuEvent::receiver().try_recv() {
                if menu_event.id == next_id {
                    let _ = tx.send(());
                } else if menu_event.id == quit_id {
                    elwt.exit();
                }
            }
        })
        .unwrap();
}
