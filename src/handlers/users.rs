use crate::util::{authenticate, create_user};
use crate::MyManagedState;
use mongodb::options::ClientOptions;
use mongodb::sync::Client;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome};
use rocket::serde::json::Json;
use rocket::Request;
use rocket::State;
use serde::{Deserialize, Serialize};

#[derive(FromForm, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AuthInfo {
    pub username: String,
    pub password: String,
}

pub struct AuthenticatedUser {
    username: String,
}

#[derive(Debug)]
pub enum LoginError {
    InvalidData,
    WrongPassword,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthenticatedUser {
    type Error = LoginError;
    async fn from_request(request: &'r Request<'_>) -> Outcome<AuthenticatedUser, LoginError> {
        let username = request.headers().get_one("username");
        let password = request.headers().get_one("password");
        // let state = request.rocket().state().unwrap();

        //TODO: not have to create another client on every request
        let mut client_options = ClientOptions::parse("mongodb://localhost:27017").unwrap();
        client_options.app_name = Some("MongoDB Client".to_string());
    
        let client = Client::with_options(client_options).unwrap();

        match (username, password) {
            (Some(u), Some(p)) => {
                let username = u.to_string();
                let password = p.to_string();
                match authenticate(&client, username.clone(), password) {
                    Ok(()) => Outcome::Success(AuthenticatedUser { username }),
                    Err(reason) => Outcome::Failure((Status::Forbidden, LoginError::WrongPassword)),
                }
            }
            _ => Outcome::Failure((Status::BadRequest, LoginError::InvalidData)),
        }
    }
}

#[post("/users/create", data = "<auth_info>")]
pub async fn create(auth_info: Json<AuthInfo>, state: &State<MyManagedState>) -> (Status, String) {
    let auth_info = auth_info.into_inner();
    match create_user(state, auth_info.username, auth_info.password) {
        Ok(()) => (Status::Created, "".to_string()),
        Err(reason) => (Status::InternalServerError, reason),
    }
}


#[post("/users/test")]
pub async fn test(state: &State<MyManagedState>, auth: AuthenticatedUser) -> (Status, String) {
  (Status::Ok, "OK".to_string())
}


