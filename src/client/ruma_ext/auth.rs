use matrix_sdk::identifiers::{DeviceId, UserId};
use ruma_client_api::r0::{account::register::Response as RegisterResponse, uiaa::UiaaResponse};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SessionObj {
    pub session: String,
}

ruma_api::ruma_api! {
    metadata {
        description: "Send session pings to the server during UIAA.",
        method: POST,
        name: "register",
        path: "/_matrix/client/r0/register?kind=user",
        rate_limited: true,
        requires_authentication: false,
    }

    request {
        pub auth: SessionObj,
    }

    response {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub access_token: Option<String>,
        pub user_id: UserId,
        pub device_id: Option<DeviceId>,
    }

    error: UiaaResponse
}

impl Into<RegisterResponse> for Response {
    fn into(self) -> RegisterResponse {
        RegisterResponse {
            access_token: self.access_token,
            user_id: self.user_id,
            device_id: self.device_id,
        }
    }
}

pub mod dummy {
    use matrix_sdk::identifiers::{DeviceId, UserId};
    use ruma_client_api::r0::{
        account::register::Response as RegisterResponse, uiaa::UiaaResponse,
    };

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
    pub struct Dummy {
        #[serde(rename = "type")]
        pub ev_type: String,
        pub session: String,
    }

    ruma_api::ruma_api! {
        metadata {
            description: "Send session pings to the server during UIAA.",
            method: POST,
            name: "register",
            path: "/_matrix/client/r0/register?kind=user",
            rate_limited: true,
            requires_authentication: false,
        }

        request {
            pub auth: Dummy,
        }

        response {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub access_token: Option<String>,
            pub user_id: UserId,
            pub device_id: Option<DeviceId>,
        }

        error: UiaaResponse
    }

    impl Into<RegisterResponse> for Response {
        fn into(self) -> RegisterResponse {
            RegisterResponse {
                access_token: self.access_token,
                user_id: self.user_id,
                device_id: self.device_id,
            }
        }
    }
}
