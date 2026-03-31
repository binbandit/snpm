use super::super::helpers::current_timestamp;

use serde::Serialize;

#[derive(Serialize)]
pub(super) struct CouchUserBody {
    _id: String,
    name: String,
    password: String,
    #[serde(rename = "type")]
    user_type: &'static str,
    roles: Vec<String>,
    date: String,
}

pub(super) fn couch_user_body(username: &str, password: &str) -> CouchUserBody {
    CouchUserBody {
        _id: format!("org.couchdb.user:{username}"),
        name: username.to_string(),
        password: password.to_string(),
        user_type: "user",
        roles: vec![],
        date: current_timestamp(),
    }
}
