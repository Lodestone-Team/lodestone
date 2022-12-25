use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::prelude::SNOWFLAKE_GENERATOR;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS, Copy)]
#[serde(transparent)]
pub struct Snowflake(i64);

impl Default for Snowflake {
    fn default() -> Self {
        Self(get_snowflake())
    }
}

impl Snowflake {
    pub fn new() -> Self {
        Self(get_snowflake())
    }
}

impl ToString for Snowflake {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

fn get_snowflake() -> i64 {
    SNOWFLAKE_GENERATOR.lock().unwrap().real_time_generate()
}
