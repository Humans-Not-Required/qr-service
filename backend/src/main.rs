#[macro_use]
extern crate rocket;

mod db;
mod models;
mod routes;
mod auth;
mod qr;

use rocket::fairing::AdHoc;
use rocket_cors::{AllowedOrigins, CorsOptions};

#[launch]
fn rocket() -> _ {
    // Load .env file if present (silently ignore if missing)
    let _ = dotenvy::dotenv();

    let cors = CorsOptions::default()
        .allowed_origins(AllowedOrigins::all())
        .to_cors()
        .expect("CORS configuration failed");

    rocket::build()
        .attach(cors)
        .attach(AdHoc::on_ignite("Database", |rocket| async {
            let db = db::init_db().expect("Failed to initialize database");
            rocket.manage(db)
        }))
        .mount("/api/v1", routes![
            routes::health,
            routes::openapi,
            routes::generate_qr,
            routes::decode_qr,
            routes::batch_generate,
            routes::generate_from_template,
            routes::get_history,
            routes::get_qr_by_id,
            routes::delete_qr,
            routes::list_keys,
            routes::create_key,
            routes::delete_key,
        ])
}
