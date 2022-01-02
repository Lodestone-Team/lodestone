use serde::{Serialize, Deserialize};

#[derive(FromForm, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AuthInfo {
  pub username: String,
  pub password_hash: String
}


// #[post("/users/create", format="json", data="<auth_info>")]
// fn create(db: State<String>, create_info: Json<AuthInfo>)
//   -> Json<i32> {
//     let user: User = User
//         { name: create_info.name.clone(),
//           email: create_info.email.clone(),
//           age: create_info.age};
//     let connection = ...;
//     let user_entity: UserEntity = diesel::insert_into(users::table)...
//     …
// }