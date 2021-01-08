#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate s3;

use actix_web::{get, delete, web, App, HttpResponse, HttpServer, Responder};
use s3::creds::Credentials;
use s3::region::Region;
use s3::bucket::Bucket;
use std::env;

mod auth;

use crate::auth::Token;


fn get_file_limit() -> usize {
    let limit: usize = env::var("FILE_LIMIT").expect("No FILE_LIMIT in .env").parse().expect("FILE_LIMIT is not a number");
    limit
}

async fn get_file_amount(user_id: &String, bucket: &Bucket) -> usize {
    let list_result = bucket.list(
        format!("{}/", user_id),
        Some(String::from("/"))).await.expect("Could not get list of files");
    let file_amount: usize = list_result.into_iter().map(|result| {
        result.contents.len()
    }).sum();
    file_amount
}

async fn has_exceeded_limit(user_id: &String, bucket: &Bucket) -> bool {
    let limit = get_file_limit();
    let file_amount = get_file_amount(user_id, bucket).await;
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
async fn get_limit(bucket: web::Data<Bucket>, token: Token) -> impl Responder {
    HttpResponse::Ok().json(&UserLimit {
        used: get_file_amount(&token.sub, bucket.get_ref()).await,
        limit: get_file_limit(),
    })
}

#[get("/put/{file_name}")]
async fn request_file_upload(web::Path(file_name): web::Path<String>, token: Token, bucket: web::Data<Bucket>) -> impl Responder {
    if !valid_filename(&*file_name) {
        return HttpResponse::BadRequest().body("InvalidFilename");
    }
    if !file_name.eq("avatar.jpg") && has_exceeded_limit(&token.sub, &bucket).await {
        return HttpResponse::BadRequest().body("ExceededLimit");
    }
    let url = bucket.presign_put(format!("/{}/{}", token.sub, file_name).to_string(), 5000).unwrap();
    HttpResponse::Ok().body(url)
}

#[get("/get/{file_name}")]
async fn request_file_download(web::Path(file_name): web::Path<String>, token: Token, bucket: web::Data<Bucket>) -> impl Responder {
    if !valid_filename(&*file_name) {
        return HttpResponse::BadRequest().body("InvalidFilename");
    }
    let url = bucket.presign_get(format!("/{}/{}", token.sub, file_name).to_string(), 60).unwrap();
    HttpResponse::Ok().body(url)
}

#[delete("/delete/{file_name}")]
async fn delete_file(web::Path(file_name): web::Path<String>, token: Token, bucket: web::Data<Bucket>) -> impl Responder {
    if !valid_filename(&*file_name) {
        return HttpResponse::BadRequest().body("InvalidFilename");
    }
    let result = bucket.delete_object(format!("/{}/{}", token.sub, file_name).to_string()).await;
    match result {
        Ok(status) => {
            println!("{}", status.1);
            return match status.1 {
                204 => {
                    HttpResponse::Ok().body("DeleteSuccess")
                }
                404 => {
                    HttpResponse::Ok().body("DeleteSuccess")
                }
                _ => {
                    HttpResponse::InternalServerError().body("UnknownError")
                }
            }
        }
        Err(error) => {
            println!("{}", error.to_string());
            return HttpResponse::InternalServerError().body("UnknownErrorOccurred");
        }
    }
}

#[delete("/delete/all")]
async fn delete_all(token: Token, bucket: web::Data<Bucket>) -> impl Responder {
    let list_result = bucket.list(format!("/{}/", token.sub), Some(String::from("/"))).await.expect("Could not get list of files");
    for result in list_result {
        for file in result.contents {
            let delete_result = bucket.delete_object(file.key).await;
            match delete_result {
                Ok(status) => {
                    println!("{}", status.1);
                    match status.1 {
                        204 => {}
                        404 => {
                            return HttpResponse::BadRequest().body("FileNotFound");
                        }
                        _ => {
                            return HttpResponse::InternalServerError().body("UnknownErrorOccurred");
                        }
                    }
                }
                Err(error) => {
                    println!("{}", error.to_string());
                    return HttpResponse::InternalServerError().body("UnknownErrorOccurred");
                }
            }
        }
    }
    return HttpResponse::Ok().body("DeleteSuccess");
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    let address = env::var("ADDRESS").expect("No ADDRESS specified in .env");
    let bucket = get_bucket().await;
    HttpServer::new(move || {
        App::new()
            .data(bucket.clone())
            .service(health)
            .service(get_limit)
            .service(request_file_upload)
            .service(request_file_download)
            .service(delete_file)
            .service(delete_all)
    }).bind(address)?
        .run().await
}

async fn get_bucket() -> Bucket {
    let region = Region::Custom {
        region: "us-east-1".into(),
        endpoint: env::var("S3_HOST").expect("No S3_HOST specified in .env"),
    };
    let access_key = env::var("S3_ACCESS_KEY").expect("No S3_ACCESS_KEY in .env");
    let secret_key = env::var("S3_SECRET_KEY").expect("No S3_SECRET_KEY in .env");
    let credentials = Credentials::new(
        Some(&*access_key),
        Some(&*secret_key),
        None,
        None,
        None).await.unwrap();
    let bucket_name = env::var("BUCKET_NAME").expect("No BUCKET_NAME in .env");
    Bucket::new_with_path_style(&*bucket_name, region, credentials).expect("Cant connect to bucket ")
}
