/*
# NOTES

- The Server has the following endpoints:
    - `/{task_id}`
    - `/annotation`
- The server listens under:
    - `/api/task`
- The server listens on:
    - Port 8080


*/

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Result};
use serde::Deserialize;

#[allow(non_camel_case_types)]
#[derive(Deserialize)]
struct NewTask {
    instruction: String,
    attachment_type: Option<String>,
    attachment: String,
    objects_to_annotate: Vec<String>,
    with_labels: bool,
}

#[post("annotation")]
async fn annotation(new_task: web::Json<NewTask>) -> Result<String> {
    Ok("Welcome!".to_string())
}

#[actix_web::main]
async fn start_http_server() -> Result<(), std::io::Error> {
    HttpServer::new(|| App::new().service(annotation))
        .bind(("0.0.0.0", 8080))?
        .run()
        .await
}

fn main() {
    println!("Hello, world!");
}
