use std::collections::BTreeMap;
use std::time::SystemTime;

use serde_json::Value as JsonValue;

use matrix_sdk::identifiers::{EventId, RoomId, UserId};

pub mod auth;
pub mod message;
pub mod reaction;

pub use message::ExtraMessageEventContent;
pub use reaction::ExtraReactionEventContent;

pub type RumaUnsupportedEvent = RumaUnsupportedRoomEvent<ExtraRoomEventContent>;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type")]
pub enum ExtraRoomEventContent {
    #[serde(rename = "m.room.message")]
    Message { content: ExtraMessageEventContent },
    #[serde(rename = "m.reaction")]
    Reaction { content: ExtraReactionEventContent },
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(bound = "C: serde::de::DeserializeOwned + serde::Serialize")]
pub struct RumaUnsupportedRoomEvent<C: serde::de::DeserializeOwned + serde::Serialize> {
    /// The event's content.
    #[serde(flatten)]
    pub content: C,

    /// The unique identifier for the event.
    pub event_id: EventId,

    /// Time on originating homeserver when this event was sent.
    #[serde(with = "ms_since_unix_epoch")]
    pub origin_server_ts: SystemTime,

    /// The unique identifier for the room associated with this event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<RoomId>,

    /// The unique identifier for the user who sent this event.
    pub sender: UserId,

    /// Additional key-value pairs not signed by the homeserver.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub unsigned: BTreeMap<String, JsonValue>,
}

/// Taken from ruma_serde as opposed to adding a dependency for two functions.
///
/// Converts `js_int::UInt` to a `SystemTime` when deserializing and vice versa when
/// serializing.
mod ms_since_unix_epoch {
    use std::{
        convert::TryFrom,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use js_int::UInt;
    use serde::{
        de::{Deserialize, Deserializer},
        ser::{Error, Serialize, Serializer},
    };

    /// Serialize a SystemTime.
    ///
    /// Will fail if integer is greater than the maximum integer that can be unambiguously represented
    /// by an f64.
    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // If this unwrap fails, the system this is executed is completely broken.
        let time_since_epoch = time.duration_since(UNIX_EPOCH).unwrap();
        match UInt::try_from(time_since_epoch.as_millis()) {
            Ok(uint) => uint.serialize(serializer),
            Err(err) => Err(S::Error::custom(err)),
        }
    }

    /// Deserializes a SystemTime.
    ///
    /// Will fail if integer is greater than the maximum integer that can be unambiguously represented
    /// by an f64.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = UInt::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + Duration::from_millis(millis.into()))
    }
}

#[test]
fn test_message_edit_event() {
    use matrix_sdk::events::EventJson;

    let ev = serde_json::from_str::<EventJson<RumaUnsupportedEvent>>(include_str!(
        "../../../test_data/message_edit.json"
    ))
    .unwrap()
    .deserialize()
    .unwrap();

    let json = serde_json::to_string_pretty(&ev).unwrap();
    assert_eq!(
        ev,
        serde_json::from_str::<EventJson<RumaUnsupportedEvent>>(&json)
            .unwrap()
            .deserialize()
            .unwrap()
    )
}

#[test]
fn test_reaction_event() {
    use matrix_sdk::events::EventJson;

    let ev = serde_json::from_str::<EventJson<RumaUnsupportedEvent>>(include_str!(
        "../../../test_data/reaction.json"
    ))
    .unwrap()
    .deserialize()
    .unwrap();

    let json = serde_json::to_string_pretty(&ev).unwrap();
    assert_eq!(
        ev,
        serde_json::from_str::<EventJson<RumaUnsupportedEvent>>(&json)
            .unwrap()
            .deserialize()
            .unwrap()
    )
}
