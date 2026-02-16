use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
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

/// Decode a logo from base64 (supports data URI or raw base64).
/// Returns the raw image bytes.
pub fn decode_logo_base64(logo: &str) -> Result<Vec<u8>, String> {
    // Strip data URI prefix if present (e.g., "data:image/png;base64,...")
    let b64_data = if let Some(comma_pos) = logo.find(',') {
        let prefix = &logo[..comma_pos];
        if prefix.starts_with("data:") {
            &logo[comma_pos + 1..]
        } else {
            logo
        }
    } else {
        logo
    };

    BASE64
        .decode(b64_data.trim())
        .map_err(|e| format!("Invalid base64 logo data: {}", e))
}

/// Overlay a logo image at the center of a QR code PNG.
/// `logo_data` is raw image bytes (PNG, JPEG, etc.).
/// `logo_pct` is the percentage of the QR code size the logo should occupy (5-40).
/// A white rounded-rect background with padding is placed behind the logo.
pub fn overlay_logo_png(qr_png: &[u8], logo_data: &[u8], logo_pct: u8) -> Result<Vec<u8>, String> {
    let mut qr_img = image::load_from_memory(qr_png)
        .map_err(|e| format!("Failed to load QR image: {}", e))?
        .to_rgba8();

    let logo_img = image::load_from_memory(logo_data)
        .map_err(|e| format!("Failed to load logo image: {}", e))?
        .to_rgba8();

    let qr_size = qr_img.width().min(qr_img.height());
    let pct = (logo_pct as u32).clamp(5, 40);
    let logo_target = (qr_size * pct) / 100;

    // Resize logo to fit within target size, preserving aspect ratio
    let (lw, lh) = (logo_img.width(), logo_img.height());
    let scale = (logo_target as f64 / lw as f64).min(logo_target as f64 / lh as f64);
    let new_w = (lw as f64 * scale).round() as u32;
    let new_h = (lh as f64 * scale).round() as u32;

    let resized = image::imageops::resize(&logo_img, new_w, new_h, image::imageops::FilterType::Lanczos3);

    // Calculate center position
    let padding = (new_w.max(new_h) as f64 * 0.15).round() as u32; // 15% padding
    let bg_w = new_w + padding * 2;
    let bg_h = new_h + padding * 2;
    let bg_x = (qr_img.width().saturating_sub(bg_w)) / 2;
    let bg_y = (qr_img.height().saturating_sub(bg_h)) / 2;
    let logo_x = (qr_img.width().saturating_sub(new_w)) / 2;
    let logo_y = (qr_img.height().saturating_sub(new_h)) / 2;

    // Draw white background with rounded corners behind logo
    let corner_r = (bg_w.min(bg_h) as f64 * 0.15).round() as u32;
    for dy in 0..bg_h {
        for dx in 0..bg_w {
            let ix = bg_x + dx;
            let iy = bg_y + dy;
            if ix < qr_img.width() && iy < qr_img.height() {
                // Check if pixel is inside rounded rect
                if is_inside_rounded_rect(dx, dy, bg_w, bg_h, corner_r) {
                    qr_img.put_pixel(ix, iy, Rgba([255, 255, 255, 255]));
                }
            }
        }
    }

    // Overlay the logo with alpha blending
    for (lx, ly, pixel) in resized.enumerate_pixels() {
        let ix = logo_x + lx;
        let iy = logo_y + ly;
        if ix < qr_img.width() && iy < qr_img.height() {
            let bg_pixel = qr_img.get_pixel(ix, iy);
            let blended = alpha_blend(bg_pixel, pixel);
            qr_img.put_pixel(ix, iy, blended);
        }
    }

    let mut buf = Cursor::new(Vec::new());
    qr_img
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("PNG encoding error: {}", e))?;

    Ok(buf.into_inner())
}

/// Alpha-blend foreground pixel over background pixel.
fn alpha_blend(bg: &Rgba<u8>, fg: &Rgba<u8>) -> Rgba<u8> {
    let fa = fg.0[3] as f64 / 255.0;
    let ba = bg.0[3] as f64 / 255.0;
    let out_a = fa + ba * (1.0 - fa);
    if out_a == 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let r = ((fg.0[0] as f64 * fa + bg.0[0] as f64 * ba * (1.0 - fa)) / out_a).round() as u8;
    let g = ((fg.0[1] as f64 * fa + bg.0[1] as f64 * ba * (1.0 - fa)) / out_a).round() as u8;
    let b = ((fg.0[2] as f64 * fa + bg.0[2] as f64 * ba * (1.0 - fa)) / out_a).round() as u8;
    Rgba([r, g, b, (out_a * 255.0).round() as u8])
}

/// Check if a point is inside a rounded rectangle.
fn is_inside_rounded_rect(x: u32, y: u32, w: u32, h: u32, r: u32) -> bool {
    // Check four corners
    let corners = [
        (r, r),                             // top-left
        (w.saturating_sub(r + 1), r),       // top-right
        (r, h.saturating_sub(r + 1)),       // bottom-left
        (w.saturating_sub(r + 1), h.saturating_sub(r + 1)), // bottom-right
    ];

    for &(cx, cy) in &corners {
        let in_corner_x = if cx <= r { x < r } else { x > w.saturating_sub(r + 1) };
        let in_corner_y = if cy <= r { y < r } else { y > h.saturating_sub(r + 1) };

        if in_corner_x && in_corner_y {
            let dx = x as f64 - cx as f64;
            let dy = y as f64 - cy as f64;
            if dx * dx + dy * dy > (r as f64) * (r as f64) {
                return false;
            }
        }
    }
    true
}

/// Build SVG elements for a logo overlay at the center of the QR code.
/// Returns SVG elements (white background rect + image) to be inserted before </svg>.
pub fn svg_logo_overlay(logo_data: &[u8], qr_size: u32, logo_pct: u8) -> Result<String, String> {
    // Detect MIME type from magic bytes
    let mime = if logo_data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "image/png"
    } else if logo_data.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if logo_data.starts_with(b"<svg") || logo_data.starts_with(b"<?xml") {
        "image/svg+xml"
    } else if logo_data.starts_with(b"GIF8") {
        "image/gif"
    } else if logo_data.starts_with(b"RIFF") {
        "image/webp"
    } else {
        "image/png" // fallback
    };

    let pct = (logo_pct as f64).clamp(5.0, 40.0);
    let logo_size = (qr_size as f64 * pct) / 100.0;
    let padding = logo_size * 0.15;
    let bg_size = logo_size + padding * 2.0;
    let bg_x = (qr_size as f64 - bg_size) / 2.0;
    let bg_y = (qr_size as f64 - bg_size) / 2.0;
    let logo_x = (qr_size as f64 - logo_size) / 2.0;
    let logo_y = (qr_size as f64 - logo_size) / 2.0;
    let corner_r = bg_size * 0.15;

    let b64 = BASE64.encode(logo_data);
    let data_uri = format!("data:{};base64,{}", mime, b64);

    Ok(format!(
        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" ry="{:.2}" fill="white"/>
<image x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" href="{}"/>"#,
        bg_x, bg_y, bg_size, bg_size, corner_r, corner_r,
        logo_x, logo_y, logo_size, logo_size, data_uri
    ))
}

/// Generate a PDF containing the QR code as vector graphics.
/// The `size` in options is used as the page size in points (1 pt = 1/72 inch).
/// Returns raw PDF bytes.
pub fn generate_pdf(data: &str, options: &QrOptions) -> Result<Vec<u8>, String> {
    use printpdf::*;

    let code = QrCode::with_error_correction_level(data, options.error_correction)
        .map_err(|e: QrError| format!("QR encoding error: {}", e))?;

    let modules = code.to_colors();
    let module_count = code.width() as u32;
    let quiet_zone = 4u32;
    let total_modules = module_count + quiet_zone * 2;
    let page_size_pt = options.size as f32;
    let module_size_pt = page_size_pt / total_modules as f32;

    // Convert points to mm for page dimensions (printpdf uses Mm for page size)
    let page_size_mm: Mm = Pt(page_size_pt).into();

    let mut ops: Vec<Op> = Vec::new();

    // Draw background
    let bg_r = options.bg_color[0] as f32 / 255.0;
    let bg_g = options.bg_color[1] as f32 / 255.0;
    let bg_b = options.bg_color[2] as f32 / 255.0;

    ops.push(Op::SetFillColor { col: Color::Rgb(Rgb::new(bg_r, bg_g, bg_b, None)) });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.0) });
    let mut bg_rect = Rect::from_xywh(Pt(0.0), Pt(0.0), Pt(page_size_pt), Pt(page_size_pt));
    bg_rect.mode = Some(PaintMode::Fill);
    bg_rect.winding_order = Some(WindingOrder::NonZero);
    ops.push(Op::DrawRectangle { rectangle: bg_rect });

    // Set foreground color
    let fg_r = options.fg_color[0] as f32 / 255.0;
    let fg_g = options.fg_color[1] as f32 / 255.0;
    let fg_b = options.fg_color[2] as f32 / 255.0;

    ops.push(Op::SetFillColor { col: Color::Rgb(Rgb::new(fg_r, fg_g, fg_b, None)) });

    // Draw QR modules
    // PDF coordinate system: origin at bottom-left, Y goes up
    for (y, row) in modules.chunks(module_count as usize).enumerate() {
        for (x, &module) in row.iter().enumerate() {
            if module == qrcode::Color::Dark {
                let px = (x as u32 + quiet_zone) as f32 * module_size_pt;
                // Flip Y: PDF origin is bottom-left, QR origin is top-left
                let py = page_size_pt - (y as u32 + quiet_zone + 1) as f32 * module_size_pt;

                match options.style {
                    QrStyle::Dots => {
                        // Approximate circle with polygon segments
                        let cx = px + module_size_pt / 2.0;
                        let cy = py + module_size_pt / 2.0;
                        let r = module_size_pt / 2.0;
                        let segments = 24u32;
                        let circle_points: Vec<LinePoint> = (0..segments)
                            .map(|i| {
                                let angle = 2.0 * std::f32::consts::PI * i as f32 / segments as f32;
                                LinePoint {
                                    p: Point {
                                        x: Pt(cx + r * angle.cos()),
                                        y: Pt(cy + r * angle.sin()),
                                    },
                                    bezier: false,
                                }
                            })
                            .collect();
                        let circle = Polygon {
                            rings: vec![PolygonRing { points: circle_points }],
                            mode: PaintMode::Fill,
                            winding_order: WindingOrder::NonZero,
                        };
                        ops.push(Op::DrawPolygon { polygon: circle });
                    }
                    QrStyle::Rounded => {
                        let neighbors = get_neighbors(&modules, module_count as usize, x, y);
                        let corner_r = module_size_pt * 0.35;
                        let polygon = build_pdf_rounded_rect(px, py, module_size_pt, module_size_pt, corner_r, &neighbors);
                        ops.push(Op::DrawPolygon { polygon });
                    }
                    QrStyle::Square => {
                        let mut rect = Rect::from_xywh(Pt(px), Pt(py), Pt(module_size_pt), Pt(module_size_pt));
                        rect.mode = Some(PaintMode::Fill);
                        rect.winding_order = Some(WindingOrder::NonZero);
                        ops.push(Op::DrawRectangle { rectangle: rect });
                    }
                }
            }
        }
    }

    // Build document
    let page = PdfPage::new(page_size_mm, page_size_mm, ops);
    let mut doc = PdfDocument::new("QR Code");
    doc.pages.push(page);

    let mut warnings = Vec::new();
    let pdf_bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    Ok(pdf_bytes)
}

/// Build a polygon for a rounded rectangle with selective corner rounding based on neighbors.
fn build_pdf_rounded_rect(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    neighbors: &[bool; 4], // [top, right, bottom, left]
) -> printpdf::Polygon {
    use printpdf::*;

    // In QR grid space: neighbors = [top_qr, right_qr, bottom_qr, left_qr]
    // Y is already flipped when calculating py (PDF bottom-left origin).
    // PDF "top" of the rect = high Y = QR top neighbor
    let tl = if !neighbors[0] && !neighbors[3] { r } else { 0.0 };
    let tr = if !neighbors[0] && !neighbors[1] { r } else { 0.0 };
    let br = if !neighbors[2] && !neighbors[1] { r } else { 0.0 };
    let bl = if !neighbors[2] && !neighbors[3] { r } else { 0.0 };

    // If no rounding, return simple polygon rect
    if tl == 0.0 && tr == 0.0 && br == 0.0 && bl == 0.0 {
        return Rect::from_xywh(Pt(x), Pt(y), Pt(w), Pt(h)).to_polygon();
    }

    // Build path with selective corner arcs (approximate arcs with line segments)
    let arc_segments = 8u32;
    let mut points: Vec<LinePoint> = Vec::new();

    let lp = |px: f32, py: f32| LinePoint { p: Point { x: Pt(px), y: Pt(py) }, bezier: false };

    // Bottom-left corner (bl radius)
    if bl > 0.0 {
        for i in 0..=arc_segments {
            let angle = std::f32::consts::PI + std::f32::consts::FRAC_PI_2 * i as f32 / arc_segments as f32;
            points.push(lp(x + bl + bl * angle.cos(), y + bl + bl * angle.sin()));
        }
    } else {
        points.push(lp(x, y));
    }

    // Bottom-right corner (br radius)
    if br > 0.0 {
        for i in 0..=arc_segments {
            let angle = 3.0 * std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_2 * i as f32 / arc_segments as f32;
            points.push(lp(x + w - br + br * angle.cos(), y + br + br * angle.sin()));
        }
    } else {
        points.push(lp(x + w, y));
    }

    // Top-right corner (tr radius)
    if tr > 0.0 {
        for i in 0..=arc_segments {
            let angle = std::f32::consts::FRAC_PI_2 * i as f32 / arc_segments as f32;
            points.push(lp(x + w - tr + tr * angle.cos(), y + h - tr + tr * angle.sin()));
        }
    } else {
        points.push(lp(x + w, y + h));
    }

    // Top-left corner (tl radius)
    if tl > 0.0 {
        for i in 0..=arc_segments {
            let angle = std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_2 * i as f32 / arc_segments as f32;
            points.push(lp(x + tl + tl * angle.cos(), y + h - tl + tl * angle.sin()));
        }
    } else {
        points.push(lp(x, y + h));
    }

    Polygon {
        rings: vec![PolygonRing { points }],
        mode: PaintMode::Fill,
        winding_order: WindingOrder::NonZero,
    }
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
