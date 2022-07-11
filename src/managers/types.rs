use std::fmt;

use rocket::request::FromParam;

#[derive(FromFormField)]
#[derive(Debug)]
pub enum ResourceType {
    World,
    Mod,
}

impl<'r> FromParam<'r> for ResourceType {
    type Error = &'static str;

    fn from_param(param: &'r str) -> Result<Self, Self::Error> {
        match param {
            "world" => Ok(ResourceType::World),
            "mod" => Ok(ResourceType::Mod),
            _ => Err("invalid resource type"),
        }
    }

}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
