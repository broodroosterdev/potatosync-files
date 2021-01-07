use actix_web::{ FromRequest, HttpRequest, dev, Error };
use jsonwebtoken::{decode, DecodingKey, Validation};
use futures_util::future::{ok, err, Ready};
use std::env;
use actix_web::error::ErrorUnauthorized;

impl FromRequest for Token {
    type Error = Error;
    type Future = Ready<Result<Self, Error>>;
    type Config = ();

    fn from_request(_req: &HttpRequest, _payload: &mut dev::Payload) -> Self::Future {
        let _auth = _req.headers().get("Authorization");
        return match _auth {
            Some(_) => {
                let _split: Vec<&str> = _auth.unwrap().to_str().unwrap().split("Bearer").collect();
                let token = _split[1].trim();
                let secret = env::var("JWT_SECRET").expect("Could not find JWT_SECRET");
                return match decode::<Token>(
                    token,
                    &DecodingKey::from_secret(secret.as_ref()),
                    &Validation::default(),
                ) {
                    Ok(_token) => {
                        let claims: Token = _token.claims;
                        if !claims.token_type.eq("jwt") {
                            return err(ErrorUnauthorized("Token invalid"));
                        }
                        ok(claims)
                    },
                    Err(_e) => {
                        err(ErrorUnauthorized("Token invalid"))
                    },
                }
            }
            None => err(ErrorUnauthorized("Missing Authorization header")),
        }
    }
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct Token {
    pub sub: String,
    role: String,
    #[serde(rename = "type")]
    token_type: String,
    iat: u64,
    exp: u64,
}
