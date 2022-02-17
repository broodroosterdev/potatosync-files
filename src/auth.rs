use actix_web::{dev, Error, FromRequest, HttpRequest, HttpResponse, ResponseError};
use actix_web::error::ErrorUnauthorized;
use alcoholic_jwt::{JWKS, token_kid, validate, Validation};
use derive_more::Display;
use futures_util::future::{err, ok, Ready};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
}

pub fn validate_token(token: &str) -> Result<bool, ServiceError> {
    let authority = std::env::var("AUTHORITY").expect("AUTHORITY must be set");
    let jwks = fetch_jwks(&format!("{}{}", authority.as_str(), "/protocol/openid-connect/certs"))
        .expect("failed to fetch jwks");
    let validations = vec![Validation::Issuer(authority), Validation::SubjectPresent];
    let kid = match token_kid(&token) {
        Ok(res) => res.expect("failed to decode kid"),
        Err(_) => return Err(ServiceError::JWKSFetchError),
    };
    let jwk = jwks.find(&kid).expect("Specified key not found in set");
    let res = validate(token, jwk, validations);
    return if let Err(_e) = res {
        println!("{:?}", _e);
        Ok(false)
    } else {
        Ok(true)
    };
}

fn fetch_jwks(uri: &str) -> Result<JWKS, Box<Error>> {
    let mut res = reqwest::get(uri).expect("Cant make JWKS request");
    let val = res.json::<JWKS>().expect("Cant deserialize JWKS response");
    return Ok(val);
}

impl FromRequest for Claims {
    type Error = Error;
    type Future = Ready<Result<Self, Error>>;
    type Config = ();

    fn from_request(_req: &HttpRequest, _payload: &mut dev::Payload) -> Self::Future {
        let _auth_header = _req.headers().get("Authorization");
        if let None = _auth_header {
            return err(ErrorUnauthorized("Missing token"));
        }

        let token = _auth_header.unwrap().to_str().unwrap().trim_start_matches("Bearer ");

        let validation = validate_token(token);
        if let Err(_e) = validation {
            return err(ErrorUnauthorized(_e));
        }

        let token_valid = validation.unwrap();
        if !token_valid {
            return err(ErrorUnauthorized("Token invalid"));
        }

        let splits: Vec<&str> = token.split(".").collect();
        let encoded_claims = splits[1].trim();
        let decoded_claims = base64::decode_config(encoded_claims, base64::URL_SAFE_NO_PAD).unwrap();

        let claims: serde_json::Result<Claims> = serde_json::from_slice(&*decoded_claims);
        if let Err(_e) = claims {
            return err(ErrorUnauthorized("Token invalid"));
        }

        return ok(claims.unwrap());
    }
}

#[derive(Debug, Display)]
pub enum ServiceError {
    #[display(fmt = "Internal Server Error")]
    InternalServerError,

    #[display(fmt = "BadRequest: {}", _0)]
    BadRequest(String),

    #[display(fmt = "JWKSFetchError")]
    JWKSFetchError,
}

// impl ResponseError trait allows to convert our errors into http responses with appropriate data
impl ResponseError for ServiceError {
    fn error_response(&self) -> HttpResponse {
        match self {
            ServiceError::InternalServerError => {
                HttpResponse::InternalServerError().json("Internal Server Error, Please try later")
            }
            ServiceError::BadRequest(ref message) => HttpResponse::BadRequest().json(message),
            ServiceError::JWKSFetchError => {
                HttpResponse::InternalServerError().json("Could not fetch JWKS")
            }
        }
    }
}