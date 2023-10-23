use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};

async fn prove(
    req: web::Json<server_routes::prove::ProveRequest>,
) -> Result<impl Responder, Box<dyn std::error::Error>> {
    Ok(HttpResponse::Ok().json(server_routes::prove::prove(req.into_inner()).await?))
}

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT").unwrap_or("8080".to_string());
    let listen_addr = std::env::var("PROVER_LADDR").unwrap_or(format!("0.0.0.0:{port}"));

    let app = || {
        let cors = Cors::permissive();
        App::new()
            .wrap(cors)
            .service(web::resource("/prove").route(web::post().to(prove)))
    };

    eprintln!("Listening on {}", listen_addr);

    HttpServer::new(app)
        .bind(listen_addr)
        .unwrap()
        .run()
        .await
        .unwrap();
}
