use serde_json::json;

use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_player::TPlayerManagement;
use crate::traits::Supported;

use super::Instance;

impl TPlayerManagement for Instance {
    fn get_player_count(&self) -> crate::traits::MaybeUnsupported<u32> {
        Supported(self.players.read().unwrap().get_ref().len() as u32)
    }

    fn get_max_player_count(&self) -> crate::traits::MaybeUnsupported<u32> {
        Supported(self.get_field("max-players").unwrap().parse().unwrap())
    }

    fn get_player_list(&self) -> crate::traits::MaybeUnsupported<Vec<serde_json::Value>> {
        Supported(
            self.players
                .read()
                .unwrap()
                .get_ref()
                .iter()
                .map(|name| json!({ "name": name }))
                .collect(),
        )
    }

    fn set_max_player_count(
        &mut self,
        _max_player_count: u32,
    ) -> crate::traits::MaybeUnsupported<()> {
        todo!()
    }
}
