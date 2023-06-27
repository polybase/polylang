use abi::Parser;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use base64::Engine;
use polylang::prover::{Inputs, ProgramExt};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProveRequest {
    miden_code: String,
    abi: abi::Abi,
    ctx_public_key: Option<abi::publickey::Key>,
    this: Option<serde_json::Value>,
    args: Vec<serde_json::Value>,
}

#[post("/prove")]
async fn prove(req: web::Json<ProveRequest>) -> Result<impl Responder, actix_web::Error> {
    let program = polylang::prover::compile_program(&req.abi, &req.miden_code).map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("failed to compile program: {}", e))
    })?;

    let this = req
        .this
        .clone()
        .unwrap_or(req.abi.default_this_value()?.into());

    let this_hash = polylang::prover::hash_this(
        req.abi.this_type.clone().ok_or_else(|| {
            actix_web::error::ErrorInternalServerError("ABI is missing `this` type")
        })?,
        &req.abi
            .this_type
            .as_ref()
            .ok_or_else(|| {
                actix_web::error::ErrorInternalServerError("ABI is missing `this` type")
            })?
            .parse(&this)?,
    )?;

    let inputs = Inputs {
        abi: req.abi.clone(),
        ctx_public_key: req.ctx_public_key.clone(),
        this: this.clone(),
        this_hash,
        args: req.args.clone(),
    };

    let output = polylang::prover::prove(&program, &inputs)?;

    let program_info = program.to_program_info_bytes();
    let new_this = Into::<serde_json::Value>::into(output.new_this);

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "old": {
            "this": this,
            "hash": inputs.this_hash,
        },
        "new": {
            "selfDestructed": output.self_destructed,
            "this": new_this,
            "hash": output.new_hash,
        },
        "stack": {
            "input": inputs.stack_values(),
            "output": output.stack,
        },
        "programInfo": base64::engine::general_purpose::STANDARD.encode(program_info),
        "proof": base64::engine::general_purpose::STANDARD.encode(output.proof),
        "debug": {
            "logs": output.run_output.logs(),
        }
    })))
}

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body("Polybase Prover Service")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(index).service(prove))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
