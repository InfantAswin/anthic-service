use std::env;

use actix_web::{web, App, HttpServer};

// Custom modules
mod core;

// Main loop
#[actix_web::main]
async fn main() -> std::io::Result<()> {

    // Get environment variables
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());

    // Parse port as u16
    let port = port
        .parse::<u16>()
        .expect("PORT must be a valid u16 number");

    // Start the HTTP server
    HttpServer::new(move || {
        App::new()
            // Routes for different scopes
            .route("/anthic-call", web::post().to(core::execute))
        })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
