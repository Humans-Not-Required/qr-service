#[macro_use]
extern crate rocket;

mod auth;
mod db;
mod models;
mod qr;
mod rate_limit;
mod routes;

use rocket::fairing::AdHoc;
use rocket::fs::{FileServer, Options};
use rocket_cors::{AllowedOrigins, CorsOptions};
use std::path::PathBuf;
use std::time::Duration;

#[launch]
fn rocket() -> _ {
    let _ = dotenvy::dotenv();

    let window_secs: u64 = std::env::var("RATE_LIMIT_WINDOW_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let limiter = rate_limit::RateLimiter::new(Duration::from_secs(window_secs));

    let static_dir: PathBuf = std::env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../frontend/dist"));

    let cors = CorsOptions::default()
        .allowed_origins(AllowedOrigins::all())
        .to_cors()
        .expect("CORS configuration failed");

    let mut build = rocket::build()
        .attach(cors)
        .attach(rate_limit::RateLimitHeaders)
        .attach(AdHoc::on_ignite("Database", |rocket| async {
            let db = db::init_db().expect("Failed to initialize database");
            rocket.manage(db)
        }))
        .manage(limiter)
        .mount(
            "/api/v1",
            routes![
                routes::health,
                routes::openapi,
                routes::llms_txt,
                // Stateless QR (no auth)
                routes::generate_qr,
                routes::decode_qr,
                routes::batch_generate,
                routes::generate_from_template,
                // Tracked QR (per-resource token auth)
                routes::create_tracked_qr,
                routes::get_tracked_qr_stats,
                routes::delete_tracked_qr,
            ],
        )
        .mount(
            "/",
            routes![
                routes::redirect_short_url,
                routes::view_qr,
                routes::root_llms_txt,
                routes::skills_index,
                routes::skills_skill_md,
            ],
        );

    if static_dir.is_dir() {
        println!("📦 Serving frontend from: {}", static_dir.display());
        build = build
            .mount("/", FileServer::new(&static_dir, Options::Index))
            .mount("/", routes![routes::spa_fallback]);
    } else {
        println!(
            "⚠️  Frontend directory not found: {} (API-only mode)",
            static_dir.display()
        );
    }

    build
}
