use image::{ImageBuffer, Rgba, RgbaImage};
use qrcode::types::QrError;
use qrcode::EcLevel;
use qrcode::QrCode;
use std::io::Cursor;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QrStyle {
    Square,
    Rounded,
    Dots,
}

impl QrStyle {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "rounded" => QrStyle::Rounded,
            "dots" => QrStyle::Dots,
            _ => QrStyle::Square,
        }
    }
}

pub struct QrOptions {
    pub size: u32,
    pub fg_color: [u8; 4],
    pub bg_color: [u8; 4],
    pub error_correction: EcLevel,
    pub style: QrStyle,
}

pub fn parse_hex_color(hex: &str) -> Result<[u8; 4], String> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 && hex.len() != 8 {
        return Err(format!("Invalid hex color: #{}", hex));
    }

    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|e| e.to_string())?;
    let a = if hex.len() == 8 {
        u8::from_str_radix(&hex[6..8], 16).map_err(|e| e.to_string())?
    } else {
        255
    };

    Ok([r, g, b, a])
}

pub fn parse_ec_level(level: &str) -> EcLevel {
    match level.to_uppercase().as_str() {
        "L" => EcLevel::L,
        "M" => EcLevel::M,
        "Q" => EcLevel::Q,
        "H" => EcLevel::H,
        _ => EcLevel::M,
    }
}

pub fn generate_png(data: &str, options: &QrOptions) -> Result<Vec<u8>, String> {
    let code = QrCode::with_error_correction_level(data, options.error_correction)
        .map_err(|e: QrError| format!("QR encoding error: {}", e))?;

    let modules = code.to_colors();
    let module_count = code.width() as u32;

    // Calculate module size to fit requested image size
    let quiet_zone = 4u32; // Standard quiet zone
    let total_modules = module_count + quiet_zone * 2;
    let module_size = (options.size / total_modules).max(1);
    let actual_size = total_modules * module_size;

    let fg = Rgba(options.fg_color);
    let bg = Rgba(options.bg_color);

    let mut img: RgbaImage = ImageBuffer::from_pixel(actual_size, actual_size, bg);

    for (y, row) in modules.chunks(module_count as usize).enumerate() {
        for (x, &module) in row.iter().enumerate() {
            if module == qrcode::Color::Dark {
                let px = (x as u32 + quiet_zone) * module_size;
                let py = (y as u32 + quiet_zone) * module_size;

                match options.style {
                    QrStyle::Dots => {
                        draw_circle_module(&mut img, px, py, module_size, fg, actual_size);
                    }
                    QrStyle::Rounded => {
                        let neighbors = get_neighbors(&modules, module_count as usize, x, y);
                        draw_rounded_module(
                            &mut img,
                            px,
                            py,
                            module_size,
                            fg,
                            actual_size,
                            &neighbors,
                        );
                    }
                    QrStyle::Square => {
                        for dy in 0..module_size {
                            for dx in 0..module_size {
                                if px + dx < actual_size && py + dy < actual_size {
                                    img.put_pixel(px + dx, py + dy, fg);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("PNG encoding error: {}", e))?;

    Ok(buf.into_inner())
}

/// Neighbor flags: [top, right, bottom, left]
fn get_neighbors(modules: &[qrcode::Color], width: usize, x: usize, y: usize) -> [bool; 4] {
    let is_dark =
        |mx: usize, my: usize| -> bool { modules[my * width + mx] == qrcode::Color::Dark };
    [
        y > 0 && is_dark(x, y - 1),         // top
        x + 1 < width && is_dark(x + 1, y), // right
        y + 1 < width && is_dark(x, y + 1), // bottom (QR is square, width==height)
        x > 0 && is_dark(x - 1, y),         // left
    ]
}

/// Draw a filled circle inscribed in the module cell.
fn draw_circle_module(
    img: &mut RgbaImage,
    px: u32,
    py: u32,
    module_size: u32,
    color: Rgba<u8>,
    img_size: u32,
) {
    let center_x = px as f64 + module_size as f64 / 2.0;
    let center_y = py as f64 + module_size as f64 / 2.0;
    let radius = module_size as f64 / 2.0;
    let r_sq = radius * radius;

    for dy in 0..module_size {
        for dx in 0..module_size {
            let ix = px + dx;
            let iy = py + dy;
            if ix < img_size && iy < img_size {
                // Use pixel center for distance check (smoother edges)
                let dist_x = ix as f64 + 0.5 - center_x;
                let dist_y = iy as f64 + 0.5 - center_y;
                if dist_x * dist_x + dist_y * dist_y <= r_sq {
                    img.put_pixel(ix, iy, color);
                }
            }
        }
    }
}

/// Draw a module with selectively rounded corners based on neighbors.
/// A corner is rounded only when BOTH adjacent sides have no neighbor.
fn draw_rounded_module(
    img: &mut RgbaImage,
    px: u32,
    py: u32,
    module_size: u32,
    color: Rgba<u8>,
    img_size: u32,
    neighbors: &[bool; 4], // [top, right, bottom, left]
) {
    let radius = (module_size as f64 * 0.35).max(1.0); // Corner radius ~35% of module
    let r_sq = radius * radius;

    // Which corners should be rounded? Only round if both adjacent edges are free.
    let round_tl = !neighbors[0] && !neighbors[3]; // no top, no left
    let round_tr = !neighbors[0] && !neighbors[1]; // no top, no right
    let round_bl = !neighbors[2] && !neighbors[3]; // no bottom, no left
    let round_br = !neighbors[2] && !neighbors[1]; // no bottom, no right

    for dy in 0..module_size {
        for dx in 0..module_size {
            let ix = px + dx;
            let iy = py + dy;
            if ix >= img_size || iy >= img_size {
                continue;
            }

            // Check if this pixel falls in a rounded-off corner
            let in_tl = dx as f64 <= radius && dy as f64 <= radius;
            let in_tr = (module_size - 1 - dx) as f64 <= radius && dy as f64 <= radius;
            let in_bl = dx as f64 <= radius && (module_size - 1 - dy) as f64 <= radius;
            let in_br =
                (module_size - 1 - dx) as f64 <= radius && (module_size - 1 - dy) as f64 <= radius;

            let mut draw = true;

            if round_tl && in_tl {
                let cx = px as f64 + radius;
                let cy = py as f64 + radius;
                let dist_x = ix as f64 + 0.5 - cx;
                let dist_y = iy as f64 + 0.5 - cy;
                if dist_x * dist_x + dist_y * dist_y > r_sq {
                    draw = false;
                }
            }
            if round_tr && in_tr {
                let cx = (px + module_size) as f64 - radius;
                let cy = py as f64 + radius;
                let dist_x = ix as f64 + 0.5 - cx;
                let dist_y = iy as f64 + 0.5 - cy;
                if dist_x * dist_x + dist_y * dist_y > r_sq {
                    draw = false;
                }
            }
            if round_bl && in_bl {
                let cx = px as f64 + radius;
                let cy = (py + module_size) as f64 - radius;
                let dist_x = ix as f64 + 0.5 - cx;
                let dist_y = iy as f64 + 0.5 - cy;
                if dist_x * dist_x + dist_y * dist_y > r_sq {
                    draw = false;
                }
            }
            if round_br && in_br {
                let cx = (px + module_size) as f64 - radius;
                let cy = (py + module_size) as f64 - radius;
                let dist_x = ix as f64 + 0.5 - cx;
                let dist_y = iy as f64 + 0.5 - cy;
                if dist_x * dist_x + dist_y * dist_y > r_sq {
                    draw = false;
                }
            }

            if draw {
                img.put_pixel(ix, iy, color);
            }
        }
    }
}

pub fn generate_svg(data: &str, options: &QrOptions) -> Result<String, String> {
    let code = QrCode::with_error_correction_level(data, options.error_correction)
        .map_err(|e: QrError| format!("QR encoding error: {}", e))?;

    let modules = code.to_colors();
    let module_count = code.width() as u32;
    let quiet_zone = 4u32;
    let total_modules = module_count + quiet_zone * 2;
    let module_size = options.size as f64 / total_modules as f64;

    let fg_hex = format!(
        "#{:02x}{:02x}{:02x}",
        options.fg_color[0], options.fg_color[1], options.fg_color[2]
    );
    let bg_hex = format!(
        "#{:02x}{:02x}{:02x}",
        options.bg_color[0], options.bg_color[1], options.bg_color[2]
    );

    let mut svg = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {size} {size}" width="{size}" height="{size}">
<rect width="{size}" height="{size}" fill="{bg}"/>
"#,
        size = options.size,
        bg = bg_hex,
    );

    for (y, row) in modules.chunks(module_count as usize).enumerate() {
        for (x, &module) in row.iter().enumerate() {
            if module == qrcode::Color::Dark {
                let px = (x as u32 + quiet_zone) as f64 * module_size;
                let py = (y as u32 + quiet_zone) as f64 * module_size;

                match options.style {
                    QrStyle::Dots => {
                        let cx = px + module_size / 2.0;
                        let cy = py + module_size / 2.0;
                        let r = module_size / 2.0;
                        svg.push_str(&format!(
                            r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{}"/>"#,
                            cx, cy, r, fg_hex
                        ));
                    }
                    QrStyle::Rounded => {
                        let neighbors = get_neighbors(&modules, module_count as usize, x, y);
                        let corner_r = module_size * 0.35;
                        svg.push_str(&svg_rounded_rect(
                            px,
                            py,
                            module_size,
                            module_size,
                            corner_r,
                            &fg_hex,
                            &neighbors,
                        ));
                    }
                    QrStyle::Square => {
                        svg.push_str(&format!(
                            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}"/>"#,
                            px, py, module_size, module_size, fg_hex
                        ));
                    }
                }
                svg.push('\n');
            }
        }
    }

    svg.push_str("</svg>");
    Ok(svg)
}

/// Generate an SVG rect with selectively rounded corners via an SVG path.
fn svg_rounded_rect(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    r: f64,
    fill: &str,
    neighbors: &[bool; 4], // [top, right, bottom, left]
) -> String {
    // Corner radii: only round corners where both adjacent edges are free
    let tl = if !neighbors[0] && !neighbors[3] {
        r
    } else {
        0.0
    };
    let tr = if !neighbors[0] && !neighbors[1] {
        r
    } else {
        0.0
    };
    let br = if !neighbors[2] && !neighbors[1] {
        r
    } else {
        0.0
    };
    let bl = if !neighbors[2] && !neighbors[3] {
        r
    } else {
        0.0
    };

    // If no rounding needed, use simple rect
    if tl == 0.0 && tr == 0.0 && br == 0.0 && bl == 0.0 {
        return format!(
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}"/>"#,
            x, y, w, h, fill
        );
    }

    // Build path with selective corner arcs
    format!(
        r#"<path d="M{:.2},{:.2} L{:.2},{:.2} Q{:.2},{:.2} {:.2},{:.2} L{:.2},{:.2} Q{:.2},{:.2} {:.2},{:.2} L{:.2},{:.2} Q{:.2},{:.2} {:.2},{:.2} L{:.2},{:.2} Q{:.2},{:.2} {:.2},{:.2} Z" fill="{}"/>"#,
        // Start: top-left after TL radius
        x + tl,
        y,
        // Top edge to TR
        x + w - tr,
        y,
        // TR corner
        x + w,
        y,
        x + w,
        y + tr,
        // Right edge to BR
        x + w,
        y + h - br,
        // BR corner
        x + w,
        y + h,
        x + w - br,
        y + h,
        // Bottom edge to BL
        x + bl,
        y + h,
        // BL corner
        x,
        y + h,
        x,
        y + h - bl,
        // Left edge to TL
        x,
        y + tl,
        // TL corner
        x,
        y,
        x + tl,
        y,
        // Fill
        fill
    )
}

/// Generate WiFi QR code data string
pub fn wifi_data(ssid: &str, password: &str, encryption: &str, hidden: bool) -> String {
    format!(
        "WIFI:T:{};S:{};P:{};H:{};;",
        encryption,
        ssid.replace(';', "\\;").replace(',', "\\,"),
        password.replace(';', "\\;").replace(',', "\\,"),
        if hidden { "true" } else { "false" }
    )
}

/// Generate vCard data string
pub fn vcard_data(
    name: &str,
    email: Option<&str>,
    phone: Option<&str>,
    org: Option<&str>,
    title: Option<&str>,
    url: Option<&str>,
) -> String {
    let mut vcard = String::from("BEGIN:VCARD\nVERSION:3.0\n");
    vcard.push_str(&format!("FN:{}\n", name));
    if let Some(email) = email {
        vcard.push_str(&format!("EMAIL:{}\n", email));
    }
    if let Some(phone) = phone {
        vcard.push_str(&format!("TEL:{}\n", phone));
    }
    if let Some(org) = org {
        vcard.push_str(&format!("ORG:{}\n", org));
    }
    if let Some(title) = title {
        vcard.push_str(&format!("TITLE:{}\n", title));
    }
    if let Some(url) = url {
        vcard.push_str(&format!("URL:{}\n", url));
    }
    vcard.push_str("END:VCARD");
    vcard
}
