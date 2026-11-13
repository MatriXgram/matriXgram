//! Read positions and pagination around the first unread message.
use matrix_sdk::ruma::{
    events::{
        fully_read::FullyReadEventContent,
        receipt::{ReceiptThread, ReceiptType},
    },
    OwnedEventId,
};
use matrix_sdk::{room::Receipts, Client};
use serde::{Deserialize, Serialize};
use types::chat::*;

pub async fn reply_event(
    client: &Client,
    room_raw: &str,
    event_raw: &str,
) -> Result<MatrixChatMessage, String> {
    let room_id = protocol::parse_room_id(room_raw)?;
    let room = client.get_room(&room_id).ok_or("Room is not available")?;
    let event: OwnedEventId = event_raw
        .try_into()
        .map_err(|e| format!("Invalid reply event: {e}"))?;
    let timeline_event = room.event(&event, None).await.map_err(|e| e.to_string())?;
    let (messages, _) = super::parsing::parse_message_chunk(
        &crate::media::DefaultMediaResolver,
        client,
        &room_id,
        vec![timeline_event],
        &mut super::parsing::ReactionAccumulator::new(),
    )
    .await;
    messages
        .into_iter()
        .next()
        .ok_or_else(|| "Event is not a supported room message".to_string())
}

#[derive(Serialize, Deserialize)]
struct LocalRead {
    event_id: String,
    timestamp: u64,
    read_at: u64,
    remaining: u32,
    previous_count: u32,
    latest_timestamp: Option<u64>,
}
fn key(room: &str) -> Vec<u8> {
    format!("desktop.read-position.{room}").into_bytes()
}
async fn local(client: &Client, room: &str) -> Option<LocalRead> {
    serde_json::from_slice(
        &client
            .state_store()
            .get_custom_value(&key(room))
            .await
            .ok()??,
    )
    .ok()
}
pub async fn unread_count(client: &Client, room: &str, server: u32, latest: Option<u64>) -> u32 {
    let Some(read) = local(client, room).await else {
        return server;
    };
    if server <= read.remaining {
        return server;
    }
    if latest.is_some() && latest <= read.latest_timestamp {
        return read.remaining;
    }
    if server < read.previous_count {
        return server;
    }
    read.remaining
        .saturating_add(server.saturating_sub(read.previous_count))
}
pub async fn state(client: &Client, room_raw: &str) -> Result<MatrixRoomReadState, String> {
    let id = protocol::parse_room_id(room_raw)?;
    let room = client.get_room(&id).ok_or("Room is not available")?;
    let user = client.user_id().ok_or("Not signed in")?;
    let mut receipt = room
        .load_user_receipt(ReceiptType::Read, &ReceiptThread::Unthreaded, user)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(private) = room
        .load_user_receipt(ReceiptType::ReadPrivate, &ReceiptThread::Unthreaded, user)
        .await
        .map_err(|e| e.to_string())?
    {
        if receipt.as_ref().and_then(|(_, r)| r.ts) < private.1.ts {
            receipt = Some(private);
        }
    }
    let full = room
        .account_data_static::<FullyReadEventContent>()
        .await
        .map_err(|e| e.to_string())?
        .and_then(|r| r.deserialize().ok())
        .map(|e| e.content.event_id.to_string());
    let mut event_id = receipt.as_ref().map(|(id, _)| id.to_string()).or(full);
    if let Some(read) = local(client, room_raw).await {
        let server_read_at = receipt
            .as_ref()
            .and_then(|(_, r)| r.ts)
            .map(|ts| u64::from(ts.get()))
            .unwrap_or(0);
        if read.read_at >= server_read_at {
            event_id = Some(read.event_id);
        }
    }
    let latest_timestamp = room
        .latest_event_timestamp()
        .map(|ts| i64::from(ts.get()) as u64);
    let unread_count = unread_count(
        client,
        room_raw,
        room.num_unread_messages().min(u32::MAX as u64) as u32,
        latest_timestamp,
    )
    .await;
    Ok(MatrixRoomReadState {
        room_id: room_raw.into(),
        event_id,
        unread_count,
        latest_timestamp,
    })
}
pub async fn mark(client: &Client, request: &MatrixMarkReadRequest) -> Result<(), String> {
    // Serialize receipts so a slow earlier request cannot move the marker back.
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = LOCK.lock().await;
    if local(client, &request.room_id)
        .await
        .is_some_and(|old| old.timestamp > request.timestamp)
    {
        return Ok(());
    }
    let room_id = protocol::parse_room_id(&request.room_id)?;
    let room = client.get_room(&room_id).ok_or("Room is not available")?;
    let event: OwnedEventId = request
        .event_id
        .as_str()
        .try_into()
        .map_err(|e| format!("Invalid read event: {e}"))?;
    room.send_multiple_receipts(
        Receipts::new()
            .fully_read_marker(event.clone())
            .public_read_receipt(event),
    )
    .await
    .map_err(|e| e.to_string())?;
    let value = LocalRead {
        event_id: request.event_id.clone(),
        timestamp: request.timestamp,
        read_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        remaining: request.remaining,
        previous_count: request.previous_count,
        latest_timestamp: request.latest_timestamp,
    };
    client
        .state_store()
        .set_custom_value(
            &key(&request.room_id),
            serde_json::to_vec(&value).map_err(|e| e.to_string())?,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub async fn context(
    client: &Client,
    db: &std::sync::Arc<storage::AppDb>,
    room_raw: &str,
    event_raw: &str,
) -> Result<MatrixReadContextResponse, String> {
    let room_id = protocol::parse_room_id(room_raw)?;
    let room = client.get_room(&room_id).ok_or("Room is not available")?;
    if let Some(cached) = db.load_initial_room_messages(room_raw).await? {
        let latest = room
            .latest_event_timestamp()
            .map(|ts| i64::from(ts.get()) as u64);
        let current = cached.messages.last().and_then(|message| message.timestamp) >= latest;
        if current
            && cached
                .messages
                .iter()
                .any(|m| m.event_id.as_deref() == Some(event_raw))
        {
            return Ok(MatrixReadContextResponse {
                room_id: room_raw.into(),
                messages: cached.messages,
                next_from: cached.next_from,
                next_to: None,
            });
        }
    }
    let event: OwnedEventId = event_raw
        .try_into()
        .map_err(|e| format!("Invalid read event: {e}"))?;
    let response = room
        .event_with_context(&event, true, 50u32.into(), None)
        .await
        .map_err(|e| e.to_string())?;
    let mut events: Vec<_> = response.events_after.into_iter().rev().collect();
    events.extend(response.event);
    events.extend(response.events_before);
    let (mut messages, _) = super::parsing::parse_message_chunk(
        &crate::media::DefaultMediaResolver,
        client,
        &room_id,
        events,
        &mut super::parsing::ReactionAccumulator::new(),
    )
    .await;
    messages.reverse();
    Ok(MatrixReadContextResponse {
        room_id: room_raw.into(),
        messages,
        next_from: response.prev_batch_token,
        next_to: response.next_batch_token,
    })
}
pub async fn newer(
    client: &Client,
    request: &MatrixGetChatMessagesRequest,
) -> Result<MatrixReadContextResponse, String> {
    let room_id = protocol::parse_room_id(&request.room_id)?;
    let room = client.get_room(&room_id).ok_or("Room is not available")?;
    let mut options = crate::helpers::build_messages_options(request.from.clone(), request.limit);
    options.dir = matrix_sdk::ruma::api::Direction::Forward;
    let response = room.messages(options).await.map_err(|e| e.to_string())?;
    let empty = response.chunk.is_empty();
    let (mut messages, _) = super::parsing::parse_message_chunk(
        &crate::media::DefaultMediaResolver,
        client,
        &room_id,
        response.chunk.into_iter().rev().collect(),
        &mut super::parsing::ReactionAccumulator::new(),
    )
    .await;
    messages.reverse();
    let next_to = if empty || response.end == request.from {
        None
    } else {
        response.end
    };
    Ok(MatrixReadContextResponse {
        room_id: request.room_id.clone(),
        messages,
        next_from: None,
        next_to,
    })
}
