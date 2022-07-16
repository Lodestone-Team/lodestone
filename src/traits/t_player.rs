use rocket::serde::json::serde_json;

use super::MaybeUnsupported;


pub trait TPlayerManagement {
    fn get_player_count(&self) -> MaybeUnsupported<u32> {
        MaybeUnsupported::Unsupported
    }
    fn get_max_player_count(&self) -> MaybeUnsupported<u32> {
        MaybeUnsupported::Unsupported
    }
    fn get_player_list(&self) -> MaybeUnsupported<Vec<serde_json::Value>> {
        MaybeUnsupported::Unsupported
    }

    fn set_max_player_count(&mut self, max_player_count: u32) -> MaybeUnsupported<()> {
        MaybeUnsupported::Unsupported
    }
}