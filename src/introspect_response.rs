use std::collections::HashMap;

#[derive(Deserialize)]
pub struct IntrospectionResponse {
    pub active: bool,
    pub exp: u64,
    pub iat: u64,
    pub auth_time: u64,
    pub jti: String,
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub typ: String,
    pub azp: String,
    pub session_state: String,
    pub preferred_username: String,
    pub email: String,
    pub email_verified: bool,
    pub acr: String,
    pub realm_access: RealmAccess,
    pub resource_access: HashMap<String, ResourceAccess>,
    pub scope: String,
    pub sid: String,
    pub client_id: String,
    pub username: String,
}

#[derive(Deserialize)]
pub struct RealmAccess {
    pub roles: Vec<String>,
}

#[derive(Deserialize)]
pub struct ResourceAccess {
    pub roles: Vec<String>,
}

#[derive(Deserialize)]
pub struct IntrospectionHeader {
    pub(crate) active: bool,
}