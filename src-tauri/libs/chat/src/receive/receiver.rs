use std::sync::Arc;

use storage::AppDb;
use types::{EventSink, RoomUpdateTriggerState};

use super::super::media::{CachedMediaResolver, DefaultMediaResolver};
use super::fetch::fetch_room_messages_impl;
use super::parsing::ReactionAccumulator;
use super::stream::ChatMessageStreamer;
use types::chat::{
    MatrixGetChatMessagesResponse, MatrixStreamChatMessagesRequest,
    MatrixStreamChatMessagesResponse,
};

#[derive(Clone, Copy)]
pub struct StreamRoomMessagesContext<'a> {
    pub event_sink: &'a dyn EventSink,
    pub app_db: &'a Arc<AppDb>,
    pub room_update_trigger_state: &'a RoomUpdateTriggerState,
    pub client: &'a matrix_sdk::Client,
}

pub async fn fetch_room_messages_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    from: Option<String>,
    limit: Option<u32>,
) -> Result<MatrixGetChatMessagesResponse, String> {
    // A one-shot fetch is a single page, so reactions from a previous page are
    // simply not in scope for a fresh load; the ones inside this page still
    // land on their targets.
    let mut reactions = ReactionAccumulator::new();
    fetch_room_messages_impl(
        &DefaultMediaResolver,
        client,
        room_id_raw,
        from,
        limit,
        &mut reactions,
    )
    .await
    .map(|page| page.response)
}

pub async fn stream_room_messages_from_client(
    context: StreamRoomMessagesContext<'_>,
    request: MatrixStreamChatMessagesRequest,
) -> Result<MatrixStreamChatMessagesResponse, String> {
    let client = context.client;
    let mut streamer = ChatMessageStreamer::new(context, &request);
    if matches!(
        request.load_kind,
        types::chat::MatrixMessageLoadKind::Initial
    ) {
        streamer
            .run(request, |room_id_raw, from, limit| async move {
                fetch_room_messages_impl(
                    &CachedMediaResolver,
                    client,
                    room_id_raw.as_str(),
                    from,
                    limit,
                    &mut ReactionAccumulator::new(),
                )
                .await
            })
            .await
    } else {
        streamer
            .run(request, |room_id_raw, from, limit| async move {
                fetch_room_messages_impl(
                    &DefaultMediaResolver,
                    client,
                    room_id_raw.as_str(),
                    from,
                    limit,
                    &mut ReactionAccumulator::new(),
                )
                .await
            })
            .await
    }
}

/// Paint timeline events already received by sync without a `/messages`
/// request or media downloads. Thread counts are hydrated from `/relations`
/// when the sync cache did not include bundled relation summaries. Returned in
/// chronological order for the frontend.
pub async fn cached_room_messages(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    limit: usize,
) -> Option<MatrixGetChatMessagesResponse> {
    let room_id = protocol::parse_room_id(room_id_raw).ok()?;
    let room = client.get_room(&room_id)?;
    client.event_cache().subscribe().ok()?;
    let (cache, _handles) = room.event_cache().await.ok()?;
    let events = cache.events().await.ok()?;
    if events.is_empty() {
        return None;
    }
    // Include reaction events before trimming parsed messages to the limit.
    let (mut messages, _) = super::parsing::parse_message_chunk(
        &crate::media::CachedMediaResolver,
        client,
        &room_id,
        events
            .into_iter()
            .rev()
            .take(limit.saturating_mul(4).max(100))
            .collect(),
        &mut ReactionAccumulator::new(),
    )
    .await;
    messages.truncate(limit);
    messages.reverse();
    if messages.is_empty() {
        return None;
    }
    Some(MatrixGetChatMessagesResponse {
        room_id: room_id_raw.into(),
        next_from: room.last_prev_batch(),
        messages,
    })
}
