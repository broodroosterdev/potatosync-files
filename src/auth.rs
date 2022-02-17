use actix_web::{dev, Error, FromRequest, HttpRequest};
use actix_web::error::ErrorUnauthorized;
use futures_util::future::{err, ok, Ready};

use crate::introspect_response::{IntrospectionHeader, IntrospectionResponse};

// Disable warnings
#[allow(unused_macros)]

// The debug version
#[cfg(debug_assertions)]
macro_rules! log {
    ($( $args:expr ),*) => { println!( $( $args ),* ); }
}

// Non-debug version
#[cfg(not(debug_assertions))]
macro_rules! log {
    ($( $args:expr ),*) => {()}
}

impl FromRequest for IntrospectionResponse {
    type Error = Error;
    type Future = Ready<Result<Self, Error>>;
    type Config = ();

    fn from_request(_req: &HttpRequest, _payload: &mut dev::Payload) -> Self::Future {
        let _auth_header = _req.headers().get("Authorization");
        if let None = _auth_header {
            return err(ErrorUnauthorized("Missing token"));
        }

        let token = _auth_header.unwrap().to_str().unwrap().trim_start_matches("Bearer ");

        let introspection_result = send_introspection_request(token);
        if introspection_result.is_none() {
            log!("Introspection result is none");
            return err(ErrorUnauthorized("Invalid token"));
        }
        let claims = introspection_result.unwrap();
        return ok(claims);
    }
}

fn send_introspection_request(token: &str) -> Option<IntrospectionResponse> {
    let client_id;
    if let Ok(value) = std::env::var("CLIENT_ID") {
        client_id = value;
    } else {
        eprintln!("Error: CLIENT_ID not set");
        return None;
    }

    let client_secret;
    if let Ok(value) = std::env::var("CLIENT_SECRET") {
        client_secret = value;
    } else {
        eprintln!("Error: CLIENT_SECRET not set");
        return None;
    }

    let introspection_url;
    if let Ok(value) = std::env::var("INTROSPECTION_URL") {
        introspection_url = value;
    } else {
        eprintln!("Error: INTROSPECTION_URL not set");
        return None;
    }

    let client = reqwest::blocking::Client::new();
    let response = client.post(&introspection_url)
        .basic_auth(client_id, Some(client_secret))
        .form(&[("token", token)])
        .send();

    if response.is_err() {
        eprintln!("Error: Introspection request returned error: {}", response.err().unwrap());
        return None;
    }

    let text: String = response.unwrap().text().unwrap();
    let header: IntrospectionHeader = serde_json::from_str(&text).unwrap();

    if !header.active {
        log!("Introspection result is not active");
        return None;
    }
    let json = serde_json::from_str::<IntrospectionResponse>(&text);
    if let Err(ref error) = json {
        eprintln!("Error: Unable to parse introspection response: {}", error);
        return None;
    }
    return Some(json.unwrap());
}