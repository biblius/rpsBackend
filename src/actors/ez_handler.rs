//! Every text message the `WsChatSession` stream handler receives is sent to this
//! handler for processing.
use super::chat::models::messages::{ChatMessage, CreateRoom, Join, Read};

use super::chat::session::WsChatSession;
use crate::actors::models::messages::client_message::{MessageData, ClientMessage};
use crate::actors::rps::{game::RPS, models::RPSData};
use crate::models::error::GlobalError;
use actix::prelude::*;
use actix_web_actors::ws::WebsocketContext;
use colored::Colorize;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use tracing::log::{info, warn};

/// Parses the text message to a `ClientMessage` struct and sends the appropriate message to
/// the server.
pub fn handle<T>(
    text: String,
    session: &mut WsChatSession,
    context: &mut WebsocketContext<WsChatSession>,
) where
    T: Serialize + DeserializeOwned,
{
    let header = get_header(text.clone());
    info!("{}{:?}", "GOT HEADER : ".yellow(), header);
    match header.as_ref() {
        "chat_message" => {
            let message = parse_message::<ChatMessage>(text);
            if let MessageData::ChatMessage(chat_message) = message.data.clone() {
                let client_message = ClientMessage::<ChatMessage> {
                    header: message.header.clone(),
                    data: MessageData::ChatMessage(chat_message),
                };
                session.address.do_send(client_message);
            }
        }
        "join" => {
            let message = parse_message::<Join>(text);
            if let MessageData::Join(message) = message.data {
                session
                    .address
                    .send(Join {
                        id: message.id,
                        room_id: message.room_id,
                    })
                    .into_actor(session)
                    .then(|res, _, ctx| {
                        match res {
                            Ok(messages) => {
                                info!("SENDING MESSAGES EZ : {:?}", messages);
                                ctx.text(
                                    generate_message("messages", MessageData::List(messages))
                                        .unwrap(),
                                );
                            }
                            Err(e) => warn!("SOMETHING WENT WRONG : {:?}", e),
                        }
                        fut::ready(())
                    })
                    .wait(context)
            }
        }
        "read" => {
            let message = parse_message::<ChatMessage>(text);
            if let MessageData::List::<ChatMessage>(messages) = message.data {
                session.address.do_send(Read { messages })
            }
        }
        "room" => {
            let message = parse_message::<CreateRoom>(text);
            if let MessageData::CreateRoom(CreateRoom { sender_id, name }) = message.data {
                session.address.do_send(CreateRoom { sender_id, name })
            }
        }
        "rps" => {
            let message = parse_message::<RPSData>(text);
            info!("{}{:?}", "GOT RPS MESSAGE : ".purple(), message);
            if let MessageData::RPS(msg) = message.data {
                session
                    .rps_address
                    .send(msg)
                    .into_actor(session)
                    .then(|res, _, ctx| {
                        match res {
                            Ok(rps_data) => match rps_data {
                                RPSData::None => {}
                                _ => {
                                    ctx.text(
                                        generate_message::<RPS>("rps", MessageData::RPS(rps_data))
                                            .unwrap(),
                                    );
                                }
                            },
                            Err(e) => warn!("SOMETHING WENT WRONG : {:?}", e),
                        }
                        fut::ready(())
                    })
                    .wait(context)
            }
        }
        // Obligatory sanity lol
        "lol" => context.text(
            generate_message::<String>("lel", MessageData::String(String::from("lel"))).unwrap(),
        ),
        _ => warn!("Bad message"),
    }
}

/// Generate a `ClientMessage` with the given data.
#[inline]
pub fn generate_message<T>(header: &str, data: MessageData<T>) -> Result<String, GlobalError>
where
    T: Serialize,
{
    serde_json::to_string(&ClientMessage {
        header: header.to_string(),
        data,
    })
    .map_err(|e| GlobalError::SerdeError(e))
}

/// Parses text to `ClientMessage`
#[inline]
pub fn parse_message<T: DeserializeOwned + Serialize>(message: String) -> ClientMessage<T> {
    info!("GOT MESSAGE PARSING : {}", message);
    serde_json::from_str::<ClientMessage<T>>(&message.trim()).unwrap()
}

#[inline]
pub fn get_header<'a>(s: String) -> String {
    let message: Value = serde_json::from_str(&s).unwrap();
    let header = &message["header"];
    header.as_str().expect("Couldn't parse header").to_string()
}
