use std::path::Path;

use serde::{Deserialize, Serialize};
use tokio::fs as async_fs;

use crate::error::Result;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Configs {
    device_id: String,
    db_version: usize,
}

impl Configs {
    pub(crate) async fn load() -> Result<Self> {
        let mut path = crate::RUMATUI_DIR.as_ref().unwrap().to_path_buf();
        path.push(".configs.json");

        let json = async_fs::read_to_string(path).await?;
        serde_json::from_str(&json).map_err(Into::into)
    }
}
