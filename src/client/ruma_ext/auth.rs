use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SessionObj {
    pub session: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegisterAuth {
    pub auth: SessionObj,
}
