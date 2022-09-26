use serde_json;

use super::{MaybeUnsupported, Unsupported};

pub trait TPlayerManagement {
    fn get_player_count(&self) -> MaybeUnsupported<u32> {
        Unsupported
    }
    fn get_max_player_count(&self) -> MaybeUnsupported<u32> {
        Unsupported
    }
    fn get_player_list(&self) -> MaybeUnsupported<Vec<serde_json::Value>> {
        Unsupported
    }

    fn set_max_player_count(&mut self, _max_player_count: u32) -> MaybeUnsupported<()> {
        Unsupported
    }
}
