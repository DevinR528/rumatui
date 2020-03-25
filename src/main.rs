use std::collections::HashMap;
use std::io::{self, Write};
use std::time::Duration;
use std::{convert::TryFrom, env, process::exit};
use std::sync::{Arc, RwLock};

use futures_util::stream::{StreamExt as _, TryStreamExt as _};
use matrix_sdk::{
    self,
    events::{
        collections::all::RoomEvent,
        room::message::{MessageEvent, MessageEventContent, TextMessageEventContent},
        EventResult,
    },
    AsyncClient, AsyncClientConfig, Room, SyncSettings,
};
use termion::event::{Key, Event as TermEvent, MouseEvent};
use termion::input::MouseTerminal;
use termion::raw::IntoRawMode;
use tui::backend::TermionBackend;
use tui::Terminal;
use url::Url;

mod app;
mod event;
mod ux;

use app::App;
use event::{Config, Event, EventHandle};

#[tokio::main]
async fn main() -> Result<(), failure::Error> {
    let mut args = std::env::args();
    let tick_rate = if let Some(tick) = args.find(|arg| arg.parse::<u64>().is_ok()) {
        tick.parse()?
    } else {
        250
    };

    let mut app = App::new().expect("error from `forget`");

    let events = EventHandle::with_config(Config {
        tick_rate: Duration::from_millis(tick_rate),
        exit_key: termion::event::Key::Ctrl('q'),
    });

    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let infos = read_test_info();

    let homeserver_url = infos.get("url").unwrap().to_string();
    let username = infos.get("user").unwrap().to_string();
    let password = infos.get("password").unwrap().to_string();
    login(homeserver_url, username, password).await?;

    // terminal.clear()?;
    // loop {
    //     ux::draw(&mut terminal, &mut app)?;
    //     match events.next()? {
    //         Event::Input(event) => match event {
    //             TermEvent::Key(key) => match key {

    //                 Key::Ctrl(c) if c == 'q' => app.should_quit = true,

    //                 Key::Char(c) if c == '\n' => {
    //                     // let response = client
    //                     //     .request(r0::alias::get_alias::Request {
    //                     //         room_alias: RoomAliasId::try_from(infos.get("room").unwrap().as_str())
    //                     //             .unwrap(),
    //                     //     })
    //                     //     .await?;

    //                     // let room_id = response.room_id;

    //                     // client
    //                     //     .request(r0::membership::join_room_by_id::Request {
    //                     //         room_id: room_id.clone(),
    //                     //         third_party_signed: None,
    //                     //     })
    //                     //     .await?;

    //                     // client
    //                     //     .request(r0::message::create_message_event::Request {
    //                     //         room_id,
    //                     //         event_type: EventType::RoomMessage,
    //                     //         txn_id: "1".to_owned(),
    //                     //         data: MessageEventContent::Text(TextMessageEventContent {
    //                     //             body: "Hello World!".to_owned(),
    //                     //             format: None,
    //                     //             formatted_body: None,
    //                     //             relates_to: None,
    //                     //         }),
    //                     //     })
    //                     //     .await?;
    //                 }
    //                 Key::Esc => app.should_quit = true,
    //                 _ => {}
    //             },
    //             TermEvent::Mouse(m) => match m {
    //                 MouseEvent::Press(_button, x, y) => {
    //                     terminal.set_cursor(x, y).unwrap();
    //                     println!("hello");
    //                 },
    //                 MouseEvent::Release(_, _) => {},
    //                 MouseEvent::Hold(_, _) => {},
    //             },
    //             TermEvent::Unsupported(_) => {},
    //         }
    //         Event::Tick => {
    //             app.on_tick();
    //         }
    //     }
    //     if app.should_quit {
    //         terminal.clear()?;
    //         break;
    //     }
    // }

    Ok(())
}

async fn async_cb(room: Arc<RwLock<Room>>, event: Arc<EventResult<RoomEvent>>) {
    let room = room.read().unwrap();
    let event = if let EventResult::Ok(event) = &*event {
        event
    } else {
        return;
    };
    if let RoomEvent::RoomMessage(MessageEvent {
        content: MessageEventContent::Text(TextMessageEventContent { body: msg_body, .. }),
        sender,
        ..
    }) = event
    {
        let user = room.members.get(&sender.to_string()).unwrap();
        println!(
            "{}: {}",
            user.display_name.as_ref().unwrap_or(&sender.to_string()),
            msg_body
        );
    }
}

async fn login(
    homeserver_url: String,
    username: String,
    password: String,
) -> Result<(), matrix_sdk::Error> {
    let client_config = AsyncClientConfig::new()
        .proxy("http://localhost:8080")?
        .disable_ssl_verification();
    let homeserver_url = Url::parse(&homeserver_url)?;
    let mut client = AsyncClient::new_with_config(homeserver_url, None, client_config).unwrap();

    client.add_event_callback(async_cb);

    client.login(username, password, None).await?;
    let _response = client.sync(SyncSettings::new()).await?;

    Ok(())
}

fn read_test_info() -> HashMap<String, String> {
    const KEYS: &[&str] = &["url", "user", "password", "community", "room"];
    std::fs::read_to_string("./info.txt")
        .unwrap()
        .split("\n")
        .zip(KEYS)
        .map(|(a, b)| (b.to_string(), a.to_string()))
        .collect()
}
