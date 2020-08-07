#![feature(proc_macro_hygiene, decl_macro)]
mod token;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate s3;

use s3::creds::Credentials;
use s3::region::Region;
use s3::bucket::Bucket;
use crate::token::Token;
use std::env;
use rocket::State;
use rocket::response::content::Json;

fn get_file_limit() -> usize {
    let limit: usize = env::var("IMAGE_LIMIT").expect("No FILE_LIMIT in .env").parse().expect("FILE_LIMIT is not a number");
    limit
}

fn get_file_amount(user_id: &String, bucket: &State<Bucket>) -> usize {
    let list_result = bucket.list_blocking(format!("/{}/", user_id), Some(String::from("/"))).expect("Could not get list of files");
    let file_amount: usize = list_result.into_iter().map(|(result, code)| {
        if code != 200 {
            panic!(format!("list_blocking returned {}", code));
        }
        result.contents.len()
    }).sum();
    file_amount
}

fn has_exceeded_limit(user_id: &String, bucket: &State<Bucket>) -> bool {
    let limit = get_file_limit();
    let file_amount = get_file_amount(user_id, bucket);
    return file_amount >= limit;
}

#[get("/url/<file_name>")]
fn request(file_name: String, token: Token, bucket: State<Bucket>) -> Result<String, String> {
    if !valid_filename(&*file_name) {
        return Err("Invalid filename".to_string());
    }
    if has_exceeded_limit(&token.sub, &bucket){
       return Err("Exceeded limit".to_string());
    }
    let url = bucket.presign_put(format!("/{}/{}", token.sub, file_name).to_string(), 5000).unwrap();
    println!("{}", url);
    Ok(url)
}

#[get("/file/<file_name>")]
fn get_image(file_name: String, token: Token, bucket: State<Bucket>) -> String {
    if !valid_filename(&*file_name) {
        return "Invalid filename".to_string()
    }
    let url = bucket.presign_get(format!("{}", file_name).to_string(), 5).unwrap();
    println!("{}", url);
    url
}

#[derive(Serialize, Deserialize)]
struct UserLimit {
    used: usize,
    limit: usize
}
#[get("/limit")]
fn get_limit(token: Token, bucket: State<Bucket>) -> String {
    serde_json::to_string(&UserLimit {
        used: get_file_amount( & token.sub, &bucket),
        limit: get_file_limit()
    }).unwrap()
}

/// Returns `true` if `id` is a valid paste ID and `false` otherwise.
fn valid_filename(filename: &str) -> bool {
    filename.chars().all(|c| {
        (c >= 'a' && c <= 'z')
            || (c >= 'A' && c <= 'Z')
            || (c >= '0' && c <= '9') || c == '.'
    })
}

fn main() {
    dotenv::dotenv().ok();
    let bucket = get_bucket();
    rocket::ignite().mount("/", routes![request, get_image, get_limit]).manage(bucket).launch();
}

fn get_bucket() -> Bucket {
    let region = Region::Custom{
        region: "us-east-1".into(),
        endpoint: env::var("S3_HOST").expect("No S3_HOST specified in .env"),
    };
    let access_key = env::var("S3_ACCESS_KEY").expect("No S3_ACCESS_KEY in .env");
    let secret_key = env::var("S3_SECRET_KEY").expect("No S3_SECRET_KEY in .env");
    let credentials = Credentials::new_blocking(Some(&*access_key), Some(&*secret_key), None, None, None).unwrap();
    Bucket::new_with_path_style("test", region, credentials).expect("Cant connect to bucket ")
}
