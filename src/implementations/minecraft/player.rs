use crate::traits::t_player::TPlayerManagement;

use super::Instance;

impl TPlayerManagement for Instance {
    fn get_player_count(&self) -> crate::traits::MaybeUnsupported<u32> {
        todo!()
    }

    fn get_max_player_count(&self) -> crate::traits::MaybeUnsupported<u32> {
        todo!()
    }

    fn get_player_list(&self) -> crate::traits::MaybeUnsupported<Vec<serde_json::Value>> {
        todo!()
    }

    fn set_max_player_count(
        &mut self,
        _max_player_count: u32,
    ) -> crate::traits::MaybeUnsupported<()> {
        todo!()
    }
}
