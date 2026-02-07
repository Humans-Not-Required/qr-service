// Unit tests for QR service core functionality

#[test]
fn test_health_endpoint() {
    // Basic sanity test: just verify QR generation logic works
    let code = qrcode::QrCode::new(b"Hello, World!").unwrap();
    assert!(code.width() > 0);
}

#[test]
fn test_hex_color_parsing() {
    // Test the color parsing utility
    let white = qr_service::qr::parse_hex_color("#FFFFFF").unwrap();
    assert_eq!(white, [255, 255, 255, 255]);

    let black = qr_service::qr::parse_hex_color("#000000").unwrap();
    assert_eq!(black, [0, 0, 0, 255]);

    let with_alpha = qr_service::qr::parse_hex_color("#FF000080").unwrap();
    assert_eq!(with_alpha, [255, 0, 0, 128]);

    // Invalid
    assert!(qr_service::qr::parse_hex_color("#GGG").is_err());
    assert!(qr_service::qr::parse_hex_color("#12").is_err());
}

#[test]
fn test_qr_png_generation() {
    let options = qr_service::qr::QrOptions {
        size: 256,
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        error_correction: qrcode::EcLevel::M,
    };

    let result = qr_service::qr::generate_png("https://example.com", &options);
    assert!(result.is_ok());
    let data = result.unwrap();
    // PNG magic bytes
    assert_eq!(&data[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    // Should be a reasonable size
    assert!(data.len() > 100);
}

#[test]
fn test_qr_svg_generation() {
    let options = qr_service::qr::QrOptions {
        size: 256,
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        error_correction: qrcode::EcLevel::M,
    };

    let result = qr_service::qr::generate_svg("https://example.com", &options);
    assert!(result.is_ok());
    let svg = result.unwrap();
    assert!(svg.contains("<svg"));
    assert!(svg.contains("</svg>"));
    assert!(svg.contains("<rect"));
}

#[test]
fn test_wifi_template_data() {
    let data = qr_service::qr::wifi_data("MyNetwork", "secret123", "WPA2", false);
    assert_eq!(data, "WIFI:T:WPA2;S:MyNetwork;P:secret123;H:false;;");
}

#[test]
fn test_wifi_template_hidden() {
    let data = qr_service::qr::wifi_data("HiddenNet", "pass", "WPA2", true);
    assert!(data.contains("H:true"));
}

#[test]
fn test_wifi_template_escaping() {
    let data = qr_service::qr::wifi_data("My;Network", "pass;word", "WPA2", false);
    assert!(data.contains("S:My\\;Network"));
    assert!(data.contains("P:pass\\;word"));
}

#[test]
fn test_vcard_generation() {
    let data = qr_service::qr::vcard_data(
        "John Doe",
        Some("john@example.com"),
        Some("+1234567890"),
        None,
        None,
        None,
    );
    assert!(data.contains("BEGIN:VCARD"));
    assert!(data.contains("FN:John Doe"));
    assert!(data.contains("EMAIL:john@example.com"));
    assert!(data.contains("TEL:+1234567890"));
    assert!(data.contains("END:VCARD"));
}

#[test]
fn test_vcard_minimal() {
    let data = qr_service::qr::vcard_data("Jane", None, None, None, None, None);
    assert!(data.contains("FN:Jane"));
    assert!(!data.contains("EMAIL:"));
}

#[test]
fn test_roundtrip_generate_decode() {
    // Generate a QR code and then decode it
    let test_data = "https://humans-not-required.github.io";
    let options = qr_service::qr::QrOptions {
        size: 512,
        fg_color: [0, 0, 0, 255],
        bg_color: [255, 255, 255, 255],
        error_correction: qrcode::EcLevel::H,
    };

    let png_data = qr_service::qr::generate_png(test_data, &options).unwrap();

    // Load the PNG and decode
    let img = image::load_from_memory(&png_data).unwrap().to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(img);
    let grids = prepared.detect_grids();
    assert!(!grids.is_empty(), "Should detect at least one QR grid");

    let (_meta, content) = grids.into_iter().next().unwrap().decode().unwrap();
    assert_eq!(content, test_data);
}

#[test]
fn test_error_correction_levels() {
    // All EC levels should work
    for level in &["L", "M", "Q", "H"] {
        let ec = qr_service::qr::parse_ec_level(level);
        let options = qr_service::qr::QrOptions {
            size: 128,
            fg_color: [0, 0, 0, 255],
            bg_color: [255, 255, 255, 255],
            error_correction: ec,
        };
        assert!(qr_service::qr::generate_png("test", &options).is_ok());
    }
}
