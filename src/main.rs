#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use actix_web::{get, put, delete, web, App, HttpResponse, HttpServer, Responder};
use std::{env, fs};

mod auth;

use crate::auth::Token;
use actix_web::http::header;
use actix_cors::Cors;
use actix_multipart::Multipart;
use futures_util::{TryStreamExt, StreamExt};
use std::io::Write;
use actix_files::NamedFile;
use std::path::Path;


fn get_file_limit() -> usize {
    let limit: usize = env::var("FILE_LIMIT").expect("No FILE_LIMIT in .env").parse().expect("FILE_LIMIT is not a number");
    limit
}

async fn get_file_amount(user_id: &String) -> usize {
    let id = user_id.clone();
    web::block(move || {
        let location = format!("./files/{}", id);
        let path = Path::new(&location);
        if !path.is_dir(){
            fs::create_dir(path)?;
        }
        Ok::<_, std::io::Error>(fs::read_dir(path)?.collect::<Vec<_>>().len())
    }).await.unwrap()
}

async fn has_exceeded_limit(user_id: &String) -> bool {
    let limit = get_file_limit();
    let file_amount = get_file_amount(user_id).await;
    return file_amount >= limit;
}

/// Returns `true` if `id` is a valid filename and `false` otherwise.
fn valid_filename(filename: &str) -> bool {
    filename.chars().all(|c| {
        (c >= 'a' && c <= 'z')
            || (c >= 'A' && c <= 'Z')
            || (c >= '0' && c <= '9')
            || c == '.'
            || c == '-'
    })
}

async fn file_exists(path: String) -> bool {
    web::block(move || {
        Ok::<_, std::io::Error>(Path::new(path.as_str()).exists())
    }).await.unwrap()
}

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().body("Ok")
}

#[derive(Serialize, Deserialize)]
struct UserLimit {
    used: usize,
    limit: usize,
}

#[get("/limit")]
async fn get_limit(token: Token) -> impl Responder {
    HttpResponse::Ok().json(&UserLimit {
        used: get_file_amount(&token.sub).await,
        limit: get_file_limit(),
    })
}

#[put("/put/{file_name}")]
async fn file_upload(
    web::Path(file_name): web::Path<String>,
    mut payload: Multipart,
    token: Token,
) -> Result<HttpResponse, actix_web::Error> {
    if !valid_filename(&*file_name) {
        return Ok(HttpResponse::BadRequest().body("InvalidFilename"));
    }

    let account_id = token.sub.clone();
    web::block(move || {
        let path = format!("./files/{}", account_id);
        if !Path::new(path.as_str()).exists(){
            std::fs::create_dir(path.as_str())?;
        }
        Ok::<_, std::io::Error>(())
    }).await.unwrap();

    if has_exceeded_limit(&token.sub).await {
        return Ok(HttpResponse::BadRequest().body("ExceededLimit"));
    }

    while let Ok(Some(mut field)) = payload.try_next().await {
        let filepath = format!("./files/{}/{}", &token.sub, &file_name);
        // File::create is blocking operation, use threadpool
        let mut f = web::block(|| std::fs::File::create(filepath))
            .await
            .unwrap();

        // Field in turn is stream of *Bytes* object
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            // filesystem operations are blocking, we have to use threadpool
            f = web::block(move || f.write_all(&data).map(|_| f)).await?;
        }
    }
    Ok(HttpResponse::Ok().body("Uploaded"))
}

#[get("/get/{file_name}")]
async fn file_download(web::Path(file_name): web::Path<String>, token: Token) -> actix_web::Result<NamedFile> {
    if !valid_filename(&*file_name) {
        return Err(HttpResponse::BadRequest().body("InvalidFilename").into());
    }

    let path = format!("./files/{}/{}", token.sub, file_name).to_string();

    let file_exists = file_exists(path.clone()).await;

    if !file_exists {
        return Err(HttpResponse::NotFound().body("FileDoesntExist").into());
    }


    Ok(NamedFile::open(path)?)
}

#[delete("/delete/{file_name}")]
async fn delete_file(web::Path(file_name): web::Path<String>, token: Token) -> HttpResponse {
    if !valid_filename(&*file_name) {
        return HttpResponse::BadRequest().body("InvalidFilename");
    }

    let path = format!("./files/{}/{}", token.sub, file_name);

    let file_exists = file_exists(path.clone()).await;

    if !file_exists {
        return HttpResponse::BadRequest().body("FileDoesntExist");
    }


    if let Err(_) = web::block(move || Ok::<_, std::io::Error>(std::fs::remove_file(path.as_str())?)).await {
        return HttpResponse::InternalServerError().body("Could not delete file")
    }

    HttpResponse::Ok().body("DeleteSuccess")
}

#[delete("/delete/all")]
async fn delete_all(token: Token) -> impl Responder {
    let account_id = token.sub.clone();

    let file_exists = web::block(move || {
        let path = format!("./files/{}", account_id);
        Ok::<_, std::io::Error>(Path::new(path.as_str()).exists())
    }).await.unwrap();

    let account_id = token.sub.clone();

    if file_exists {
        if let Err(_) = web::block(move || {
            let path = format!("./files/{}", account_id);
            Ok::<_, std::io::Error>(std::fs::remove_dir_all(path)?)
        }).await {
            return HttpResponse::InternalServerError().body("Could not delete all files")
        }
    }
    return HttpResponse::Ok().body("DeleteAllSuccess");
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    let address = env::var("ADDRESS").expect("No ADDRESS specified in .env");
    ensure_files_dir_created();
    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "PUT", "DELETE"])
            .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
            .allowed_header(header::CONTENT_TYPE)
            .max_age(3600);
        App::new()
            .wrap(cors)
            .service(health)
            .service(get_limit)
            .service(file_upload)
            .service(file_download)
            .service(delete_all)
            .service(delete_file)
    }).bind(address.clone())?
        .run();

    println!("Server started on: {}", address);

    server.await
}

fn ensure_files_dir_created() -> () {
    if !Path::new("./files").exists() {
        std::fs::create_dir("./files").expect("Could not create directory 'files'");
    }
}
