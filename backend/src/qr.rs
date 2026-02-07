use image::{ImageBuffer, Rgba, RgbaImage};
use qrcode::QrCode;
use qrcode::types::QrError;
use qrcode::EcLevel;
use std::io::Cursor;

pub struct QrOptions {
    pub size: u32,
    pub fg_color: [u8; 4],
    pub bg_color: [u8; 4],
    pub error_correction: EcLevel,
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
    
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("PNG encoding error: {}", e))?;
    
    Ok(buf.into_inner())
}

pub fn generate_svg(data: &str, options: &QrOptions) -> Result<String, String> {
    let code = QrCode::with_error_correction_level(data, options.error_correction)
        .map_err(|e: QrError| format!("QR encoding error: {}", e))?;
    
    let modules = code.to_colors();
    let module_count = code.width() as u32;
    let quiet_zone = 4u32;
    let total_modules = module_count + quiet_zone * 2;
    let module_size = options.size as f64 / total_modules as f64;
    
    let fg_hex = format!("#{:02x}{:02x}{:02x}", options.fg_color[0], options.fg_color[1], options.fg_color[2]);
    let bg_hex = format!("#{:02x}{:02x}{:02x}", options.bg_color[0], options.bg_color[1], options.bg_color[2]);
    
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
                svg.push_str(&format!(
                    r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}"/>"#,
                    px, py, module_size, module_size, fg_hex
                ));
                svg.push('\n');
            }
        }
    }
    
    svg.push_str("</svg>");
    Ok(svg)
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
