use matrix_sdk::{
    api::r0::{account::register::Response as RegisterResponse, uiaa::UiaaResponse},
    assign,
    identifiers::{DeviceId, UserId},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SessionObj {
    pub session: String,
}

ruma_api::ruma_api! {
    metadata: {
        description: "Send session pings to the server during UIAA.",
        method: POST,
        name: "register",
        path: "/_matrix/client/r0/register?kind=user",
        rate_limited: true,
        authentication: None,
    }

    request: {
        pub auth: SessionObj,
    }

    response: {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub access_token: Option<String>,
        pub user_id: UserId,
        pub device_id: Option<Box<DeviceId>>,
    }

    error: UiaaResponse
}

impl From<Response> for RegisterResponse {
    fn from(res: Response) -> RegisterResponse {
        assign!(RegisterResponse::new(res.user_id), {
            access_token: res.access_token,
            device_id: res.device_id,
        })
    }
}

pub mod dummy {
    use matrix_sdk::{
        api::r0::{account::register::Response as RegisterResponse, uiaa::UiaaResponse},
        assign,
        identifiers::{DeviceId, UserId},
    };

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
    pub struct Dummy {
        #[serde(rename = "type")]
        pub ev_type: String,
        pub session: String,
    }

    ruma_api::ruma_api! {
        metadata: {
            description: "Send session pings to the server during UIAA.",
            method: POST,
            name: "register",
            path: "/_matrix/client/r0/register?kind=user",
            rate_limited: true,
            authentication: None,
        }

        request: {
            pub auth: Dummy,
        }

        response: {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub access_token: Option<String>,
            pub user_id: UserId,
            pub device_id: Option<Box<DeviceId>>,
        }

        error: UiaaResponse
    }

    impl From<Response> for RegisterResponse {
        fn from(res: Response) -> RegisterResponse {
            assign!(RegisterResponse::new(res.user_id), {
                access_token: res.access_token,
                device_id: res.device_id,
            })
        }
    }
}
