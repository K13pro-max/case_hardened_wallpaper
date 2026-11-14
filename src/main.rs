#![windows_subsystem = "windows"]

use ab_glyph::{FontRef, PxScale};
use image::{Rgb, RgbImage};
use imageproc::drawing::{draw_text_mut, text_size};
use rand::{rngs::StdRng, Rng, SeedableRng};
use rayon::prelude::*;
use std::{
    env,
    ffi::OsStr,
    fs,
    os::windows::ffi::OsStrExt,
    path::PathBuf,
    thread,
    time::Duration,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_SETDESKWALLPAPER,
};

const W: u32 = 2560;
const H: u32 = 1440;

const PROGI: [f32; 11] = [0.0, 0.10, 0.22, 0.30, 0.38, 0.50, 0.62, 0.70, 0.78, 0.90, 1.0];
const KOLORY: [(f32, f32, f32); 11] = [
    (20.0, 105.0, 225.0),  // 0.00 - Głęboki niebieski
    (35.0, 125.0, 235.0),  // 0.10 - Rozszerzony niebieski
    (138.0, 65.0, 168.0),  // 0.22 - Szeroki fioletowy
    (212.0, 168.0, 78.0),  // 0.30 - Złoty
    (185.0, 136.0, 64.0),  // 0.38 - Ciepłe złoto
    (165.0, 170.0, 178.0), // 0.50 - Czysty srebrny
    (185.0, 136.0, 64.0),  // 0.62 - Ciepłe złoto
    (212.0, 168.0, 78.0),  // 0.70 - Złoty
    (138.0, 65.0, 168.0),  // 0.78 - Szeroki fioletowy
    (35.0, 125.0, 235.0),  // 0.90 - Rozszerzony niebieski
    (20.0, 105.0, 225.0),  // 1.00 - Głęboki niebieski
];

fn interp_color(t: f32) -> (f32, f32, f32) {
    for i in 0..(PROGI.len() - 1) {
        let p0 = PROGI[i];
        let p1 = PROGI[i + 1];
        if t >= p0 && t <= p1 {
            let ratio = (t - p0) / (p1 - p0);
            let (r0, g0, b0) = KOLORY[i];
            let (r1, g1, b1) = KOLORY[i + 1];
            return (
                r0 + ratio * (r1 - r0),
                g0 + ratio * (g1 - g0),
                b0 + ratio * (b1 - b0),
            );
        }
    }
    KOLORY[0]
}

fn generate_wallpaper(output_path: &PathBuf) {
    let mut base_rng = rand::thread_rng();
    let seed: u64 = base_rng.gen_range(1..1000);
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

    // 1. Obliczamy falę wielowątkowo (wszystkie rdzenie CPU)
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

    // Znajdujemy min i max do normalizacji
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

    // 2. Mapowanie na RGB + szum metalu
    let mut buffer = vec![0u8; (W * H * 3) as usize];
    buffer
        .par_chunks_mut((W * 3) as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let mut row_rng = StdRng::seed_from_u64(seed.wrapping_add(y as u64 * 7919));
            for x in 0..(W as usize) {
                let val = raw_fala[y * (W as usize) + x];
                let t = ((val - min_val) / span).clamp(0.0, 1.0);
                let (r, g, b) = interp_color(t);

                // Przybliżenie rozkładu normalnego dla faktury metalu
                let u_sum: f32 = (0..6).map(|_| row_rng.gen::<f32>()).sum::<f32>() - 3.0;
                let grain = u_sum * 5.5;

                row[x * 3] = (r + grain).clamp(0.0, 255.0) as u8;
                row[x * 3 + 1] = (g + grain).clamp(0.0, 255.0) as u8;
                row[x * 3 + 2] = (b + grain).clamp(0.0, 255.0) as u8;
            }
        });

    let mut img = RgbImage::from_raw(W, H, buffer).expect("Błąd bufora obrazu");

    // 3. Dodajemy napis w prawym GÓRNYM rogu korzystając z systemowej czcionki Arial
    if let Ok(font_data) = fs::read("C:\\Windows\\Fonts\\arial.ttf") {
        if let Ok(font) = FontRef::try_from_slice(&font_data) {
            let tekst = format!("randomly generated, pattern index: {}", seed);
            let scale = PxScale::from(24.0);
            let (tw, _th) = text_size(scale, &font, &tekst);

            // Prawy górny róg (24 px marginesu od prawej i 24 px od góry)
            let pos_x = (W - 24).saturating_sub(tw) as i32;
            let pos_y = 24;

            let czarny = Rgb::from([0u8, 0u8, 0u8]);
            draw_text_mut(&mut img, czarny, pos_x, pos_y, scale, &font, &tekst);
        }
    }

    // 4. Zapisujemy plik i ustawiamy jako tapetę Windows
    if img.save(output_path).is_ok() {
        set_windows_wallpaper(output_path);
    }
}

fn set_windows_wallpaper(path: &PathBuf) {
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
    // Zapisujemy tapetę obok pliku .exe
    let mut output_path = env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("."))
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();
    output_path.push("case_hardened.png");

    loop {
        generate_wallpaper(&output_path);
        // Czekamy 15 minut (900 sekund)
        thread::sleep(Duration::from_secs(15 * 60));
    }
}
