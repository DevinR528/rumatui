use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use matrix_sdk::{
    api::r0::{
        account::register,
        directory::get_public_rooms_filtered,
        membership::{join_room_by_id, leave_room},
        message::{get_message_events, send_message_event},
        read_marker::set_read_marker,
        session::login,
        typing::create_typing_event,
    },
    deserialized_responses::SyncResponse,
    events::{
        room::message::MessageEventContent, AnyMessageEventContent, AnySyncMessageEvent,
        AnySyncRoomEvent, AnyToDeviceEvent,
    },
    identifiers::{EventId, RoomId, UserId},
    Client, JoinedRoom, LoopCtrl, RoomState, Sas, SyncSettings,
};
use tokio::{
    runtime::Handle,
    sync::mpsc::{self, Sender},
    task::JoinHandle,
};
use uuid::Uuid;

use crate::{
    client::{event_stream::EventStream, MatrixClient},
    error::{Error, Result},
};

async fn wait_for_confirmation(client: Client, sas: Sas) {
    println!("Does the emoji match: {:?}", sas.emoji());

    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("error: unable to read user input");

    match input.trim().to_lowercase().as_ref() {
        "yes" | "true" | "ok" => {
            sas.confirm().await.unwrap();

            if sas.is_done() {
                print_result(&sas);
                print_devices(sas.other_device().user_id(), &client).await;
            }
        }
        _ => sas.cancel().await.unwrap(),
    }
}

fn print_result(sas: &Sas) {
    let device = sas.other_device();

    println!(
        "Successfully verified device {} {} {:?}",
        device.user_id(),
        device.device_id(),
        device.local_trust_state()
    );
}

async fn print_devices(user_id: &UserId, client: &Client) {
    println!("Devices of user {}", user_id);

    for device in client.get_user_devices(user_id).await.unwrap().devices() {
        println!(
            "   {:<10} {:<30} {:<}",
            device.device_id(),
            device.display_name().as_deref().unwrap_or_default(),
            device.is_trusted()
        );
    }
}

/// Requests sent from the UI portion of the app.
///
/// Each request is sent in response to some user input.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum UserRequest {
    Login(String, String),
    Register(String, String),
    SendMessage(RoomId, AnyMessageEventContent, Uuid),
    RoomMsgs(RoomId),
    AcceptInvite(RoomId),
    DeclineInvite(RoomId),
    JoinRoom(RoomId),
    LeaveRoom(RoomId),
    Typing(RoomId),
    ReadReceipt(RoomId, EventId),
    RoomSearch(String, String, Option<String>),
    UiaaPing(String),
    UiaaDummy(String),
    Quit,
}
unsafe impl Send for UserRequest {}

/// Either a `UserRequest` succeeds or fails with the given result.
#[allow(clippy::type_complexity)]
pub enum RequestResult {
    Login(Result<(HashMap<RoomId, RoomState>, login::Response)>),
    Register(Result<register::Response>),
    SendMessage(Result<send_message_event::Response>),
    RoomMsgs(Result<(get_message_events::Response, JoinedRoom)>),
    AcceptInvite(Result<join_room_by_id::Response>),
    DeclineInvite(Result<leave_room::Response>, RoomId),
    LeaveRoom(Result<leave_room::Response>, RoomId),
    JoinRoom(Result<RoomId>),
    Typing(Result<create_typing_event::Response>),
    ReadReceipt(Result<set_read_marker::Response>),
    RoomSearch(Result<get_public_rooms_filtered::Response>),
    Error(Error),
}

unsafe impl Send for RequestResult {}

/// The main task event loop.
///
/// `MatrixEventHandle` controls the `sync_forever` and user request loop.
pub struct MatrixEventHandle {
    cli_jobs: JoinHandle<Result<()>>,
    sync_jobs: JoinHandle<Result<()>>,
    start_sync: Arc<AtomicBool>,
    quit_flag: Arc<AtomicBool>,
}
unsafe impl Send for MatrixEventHandle {}

impl MatrixEventHandle {
    pub async fn new(
        stream: EventStream,
        to_app: Sender<RequestResult>,
        exec_hndl: Handle,
        homeserver: &str,
    ) -> (Self, Sender<UserRequest>) {
        let (app_sender, mut recv) = mpsc::channel(1024);

        let mut client = MatrixClient::new(homeserver).unwrap();
        client.inner.add_event_emitter(Box::new(stream)).await;

        let cli = client.inner.clone();
        // when the ui loop logs in `start_sync` releases and starts `sync_forever`
        let start_sync = Arc::from(AtomicBool::from(false));
        let quit_flag = Arc::from(AtomicBool::from(false));

        let is_sync = Arc::clone(&start_sync);
        let quitting = Arc::clone(&quit_flag);
        // this loop uses the above `AtomicBool` to signal shutdown.
        let sync_jobs = exec_hndl.spawn(async move {
            while !is_sync.load(Ordering::SeqCst) {
                if quitting.load(Ordering::SeqCst) {
                    return Ok(());
                }

                core::hint::spin_loop();
            }

            if quitting.load(Ordering::SeqCst) {
                return Ok(());
            }
            let client_ref = &cli;
            let initial_sync = Arc::new(AtomicBool::from(true));
            let initial_ref = &initial_sync;

            let set = matrix_sdk::SyncSettings::default();
            cli.sync_with_callback(set.clone(), |response| async move {
                let client = &client_ref;
                let initial = &initial_ref;

                for event in &response.to_device.events {
                    match event {
                        AnyToDeviceEvent::KeyVerificationStart(e) => {
                            let sas = client
                                .get_verification(&e.content.transaction_id)
                                .await
                                .expect("Sas object wasn't created");
                            println!(
                                "Starting verification with {} {}",
                                &sas.other_device().user_id(),
                                &sas.other_device().device_id()
                            );
                            print_devices(&e.sender, &client).await;
                            sas.accept().await.unwrap();
                        }

                        AnyToDeviceEvent::KeyVerificationKey(e) => {
                            let sas = client
                                .get_verification(&e.content.transaction_id)
                                .await
                                .expect("Sas object wasn't created");

                            tokio::spawn(wait_for_confirmation((*client).clone(), sas));
                        }

                        AnyToDeviceEvent::KeyVerificationMac(e) => {
                            let sas = client
                                .get_verification(&e.content.transaction_id)
                                .await
                                .expect("Sas object wasn't created");

                            if sas.is_done() {
                                print_result(&sas);
                                print_devices(&e.sender, &client).await;
                            }
                        }

                        _ => (),
                    }
                }

                if !initial.load(Ordering::SeqCst) {
                    for (_room_id, room_info) in response.rooms.join {
                        for event in room_info.timeline.events {
                            if let AnySyncRoomEvent::Message(event) = event {
                                match event {
                                    AnySyncMessageEvent::RoomMessage(m) => {
                                        if let MessageEventContent::VerificationRequest(_) =
                                            &m.content
                                        {
                                            let request = client
                                                .get_verification_request(&m.event_id)
                                                .await
                                                .expect("Request object wasn't created");

                                            request
                                                .accept()
                                                .await
                                                .expect("Can't accept verification request");
                                        }
                                    }
                                    AnySyncMessageEvent::KeyVerificationKey(e) => {
                                        let sas = client
                                            .get_verification(&e.content.relation.event_id.as_str())
                                            .await
                                            .expect("Sas object wasn't created");

                                        tokio::spawn(wait_for_confirmation((*client).clone(), sas));
                                    }
                                    AnySyncMessageEvent::KeyVerificationMac(e) => {
                                        let sas = client
                                            .get_verification(&e.content.relation.event_id.as_str())
                                            .await
                                            .expect("Sas object wasn't created");

                                        if sas.is_done() {
                                            print_result(&sas);
                                            print_devices(&e.sender, &client).await;
                                        }
                                    }
                                    _ => (),
                                }
                            }
                        }
                    }
                }

                initial.store(false, Ordering::SeqCst);

                LoopCtrl::Continue
            })
            .await;
            Ok(())
        });

        // this loop is shutdown with a channel message
        let cli_jobs = exec_hndl.spawn(async move {
            loop {
                let input = recv.recv().await;
                if input.is_none() {
                    return Ok(());
                }

                match input.unwrap() {
                    UserRequest::Quit => return Ok(()),
                    UserRequest::Login(u, p) => {
                        let res = client.login(&u, &p).await;
                        if let Err(e) = to_app.send(RequestResult::Login(res)).await {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::Register(u, p) => {
                        let res = client.register_user(&u, &p).await;
                        if let Err(e) = to_app.send(RequestResult::Register(res)).await {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        } else {
                            tracing::info!("start UIAA cycle");
                        }
                    }
                    UserRequest::UiaaPing(sess) => {
                        let res = client.send_uiaa_ping(sess).await;
                        if let Err(e) = to_app
                            .send(RequestResult::Register(res.map(Into::into)))
                            .await
                        {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        } else {
                            tracing::info!("ping UIAA endpoint");
                        }
                    }
                    UserRequest::UiaaDummy(sess) => {
                        let res = client.send_uiaa_dummy(sess).await;
                        if let Err(e) = to_app
                            .send(RequestResult::Register(res.map(Into::into)))
                            .await
                        {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        } else {
                            tracing::info!("sending the dummy UIAA request");
                        }
                    }
                    UserRequest::SendMessage(room, msg, uuid) => {
                        let res = client.send_message(&room, msg, uuid).await;
                        if let Err(e) = to_app.send(RequestResult::SendMessage(res)).await {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::RoomMsgs(room_id) => match client.get_messages(&room_id).await {
                        Ok(res) => {
                            if let Err(e) = to_app
                                .send(RequestResult::RoomMsgs(Ok((
                                    res,
                                    client
                                        .inner
                                        .joined_rooms()
                                        .into_iter()
                                        .find(|r| r.room_id() == &room_id)
                                        .unwrap(),
                                ))))
                                .await
                            {
                                tracing::error!("client event handler crashed {}", e);
                                panic!("client event handler crashed {}", e)
                            } else {
                                // store state after receiving past events incase a sync_forever call only found a few messages
                                // if client.store_room_state(&room_id).await.is_err() {
                                // TODO log that an error happened at some point
                                // }
                            }
                        }
                        Err(get_msg_err) => {
                            if let Err(e) = to_app.send(RequestResult::Error(get_msg_err)).await {
                                tracing::error!("client event handler crashed {}", e);
                                panic!("client event handler crashed {}", e)
                            }
                        }
                    },
                    UserRequest::RoomSearch(filter, network, tkn) => {
                        match client
                            .get_rooms_filtered(&filter, &network, tkn.as_deref())
                            .await
                        {
                            Ok(res) => {
                                if let Err(e) =
                                    to_app.send(RequestResult::RoomSearch(Ok(res))).await
                                {
                                    tracing::error!("client event handler crashed {}", e);
                                    panic!("client event handler crashed {}", e)
                                }
                            }
                            Err(err) => {
                                if let Err(e) = to_app.send(RequestResult::Error(err)).await {
                                    tracing::error!("client event handler crashed {}", e);
                                    panic!("client event handler crashed {}", e)
                                }
                            }
                        }
                    }
                    UserRequest::AcceptInvite(room_id) => {
                        let res = client.join_room_by_id(&room_id).await;
                        if let Err(e) = to_app.send(RequestResult::AcceptInvite(res)).await {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::DeclineInvite(room_id) => {
                        let res = client.leave_room(&room_id).await;
                        if let Err(e) = to_app
                            .send(RequestResult::DeclineInvite(res, room_id))
                            .await
                        {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::LeaveRoom(room_id) => {
                        let res = client.leave_room(&room_id).await;
                        if let Err(e) = to_app
                            .send(RequestResult::LeaveRoom(res, room_id.clone()))
                            .await
                        {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        } else if let Err(error) = client.forget_room(&room_id).await {
                            // forget room failed so send that to the UI
                            if let Err(e) = to_app.send(RequestResult::Error(error)).await {
                                tracing::error!("client event handler crashed {}", e);
                                panic!("client event handler crashed {}", e)
                            }
                        }
                    }
                    UserRequest::JoinRoom(room_id) => {
                        // TODO just send the result
                        match client.join_room_by_id(&room_id).await {
                            Ok(res) => {
                                let room_id = &res.room_id;
                                if let Err(e) = to_app
                                    .send(RequestResult::JoinRoom(Ok(room_id.clone())))
                                    .await
                                {
                                    tracing::error!("client event handler crashed {}", e);
                                    panic!("client event handler crashed {}", e)
                                }
                            }
                            Err(err) => {
                                if let Err(e) = to_app.send(RequestResult::JoinRoom(Err(err))).await
                                {
                                    tracing::error!("client event handler crashed {}", e);
                                    panic!("client event handler crashed {}", e)
                                }
                            }
                        }
                    }
                    UserRequest::ReadReceipt(room_id, event_id) => {
                        let res = client
                            .read_marker(&room_id, &event_id, Some(&event_id))
                            .await;
                        if let Err(e) = to_app.send(RequestResult::ReadReceipt(res)).await {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::Typing(room_id) => {
                        let res = client
                            .typing_notice(&room_id, true, Some(Duration::from_millis(3000)))
                            .await;
                        if let Err(e) = to_app.send(RequestResult::Typing(res)).await {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        }
                    }
                }
            }
        });

        (
            MatrixEventHandle {
                cli_jobs,
                sync_jobs,
                start_sync,
                quit_flag,
            },
            app_sender,
        )
    }

    /// This is called after login and initial sync to start `AsyncClient::sync_forever` loop.
    pub(crate) fn start_sync(&self) {
        self.start_sync
            .swap(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// This is called when the user quits to signal the `tokio::Runtime` to shutdown.
    pub(crate) fn quit_sync(&self) {
        self.quit_flag
            .swap(true, std::sync::atomic::Ordering::SeqCst);
    }
}
