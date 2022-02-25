use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    pub author: String,
    pub temperature_c: f32,
    pub temperature_f: f32,
    pub humidity: f32,
    pub message: String,
}
