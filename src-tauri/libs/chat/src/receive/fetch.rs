use futures_util::stream::{self, StreamExt};
use log::{debug, warn};

use crate::helpers::build_messages_options;
use crate::media::{DefaultMediaResolver, MediaResolver};
use types::chat::MatrixGetChatMessagesResponse;

use super::parsing::{parse_message_chunk, ReactionAccumulator};
use protocol::{parse_event_id, parse_room_id};

/// One page of messages as returned by the server, plus the size of the RAW
/// event chunk it was parsed from.
///
/// The raw count matters because `messages` only contains events that survived
/// parsing: reaction events, state events and undecryptable chunks are all
/// dropped. Paging has to stop on "the server had nothing left", not on "we
/// could not use anything in this page", otherwise a page of pure reactions
/// silently truncates the timeline.
pub(super) struct FetchedPage {
    pub response: MatrixGetChatMessagesResponse,
    pub raw_event_count: usize,
    /// The reaction accumulator, handed back so the caller can carry it into the
    /// next page. The target of a reaction is normally still waiting in a page
    /// that has not been fetched yet.
    pub reactions_by_target: ReactionAccumulator,
}

pub(super) async fn fetch_room_messages_impl<M: MediaResolver>(
    media_resolver: &M,
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    from: Option<String>,
    limit: Option<u32>,
    reactions_by_target: &mut ReactionAccumulator,
) -> Result<FetchedPage, String> {
    let room_id = parse_room_id(room_id_raw)?;

    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from(crate::helpers::ROOM_NOT_AVAILABLE))?;

    let response = room
        .messages(build_messages_options(from.clone(), limit))
        .await
        .map_err(|error| format!("Failed to read room messages: {error}"))?;

    let sender_names: std::collections::HashMap<String, String> = response
        .state
        .iter()
        .filter_map(|raw| raw.deserialize_as::<serde_json::Value>().ok())
        .filter(|event| {
            event.get("type").and_then(serde_json::Value::as_str) == Some("m.room.member")
        })
        .filter_map(|event| {
            Some((
                event.get("state_key")?.as_str()?.to_owned(),
                event.pointer("/content/displayname")?.as_str()?.to_owned(),
            ))
        })
        .collect();
    let raw_event_count = response.chunk.len();
    // One accumulator per pagination run, not per page: it has to outlive the
    // page a reaction was parsed in, because its target is in an older page.
    let (mut messages, mut had_utd) = parse_message_chunk(
        media_resolver,
        client,
        &room_id,
        response.chunk,
        reactions_by_target,
    )
    .await;
    let mut next_from = response.end;

    if had_utd && client.encryption().backups().are_enabled().await {
        if let Err(error) = client
            .encryption()
            .backups()
            .download_room_keys_for_room(&room_id)
            .await
        {
            warn!(
                "Failed to download backup keys for room {}: {}",
                room_id, error
            );
        } else {
            // The retry is best-effort: if it fails too, the partly
            // undecryptable first page is still the best we have, but the
            // placeholders must not be cached as if they were final.
            match room.messages(build_messages_options(from, limit)).await {
                Ok(retry_response) => {
                    let (retry_messages, retry_had_utd) = parse_message_chunk(
                        media_resolver,
                        client,
                        &room_id,
                        retry_response.chunk,
                        reactions_by_target,
                    )
                    .await;
                    messages = retry_messages;
                    had_utd = retry_had_utd;
                    next_from = retry_response.end;
                }
                Err(error) => {
                    warn!(
                        "Retrying room {} messages after downloading backup keys failed: {}",
                        room_id, error
                    );
                }
            }
        }
    }

    hydrate_thread_summaries(client, &room_id, &mut messages).await;

    for message in &mut messages {
        if let Some(name) = sender_names
            .get(&message.sender)
            .filter(|name| !name.trim().is_empty())
        {
            message.sender_display_name = Some(name.clone());
        }
    }
    debug!(
        "Fetched {} chat messages from {} raw events (utd_present={})",
        messages.len(),
        raw_event_count,
        had_utd
    );

    Ok(FetchedPage {
        response: MatrixGetChatMessagesResponse {
            room_id: room_id.to_string(),
            next_from,
            messages,
        },
        raw_event_count,
        reactions_by_target: std::mem::take(reactions_by_target),
    })
}

/// Fetch replies by their Matrix `m.thread` relation instead of scanning room
/// history. Thread replies are not guaranteed to be in the visible `/messages`
/// window, which is why a timeline-page search can return an empty thread.
pub async fn fetch_thread_messages_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    thread_root_event_id_raw: &str,
) -> Result<Vec<types::chat::MatrixChatMessage>, String> {
    use matrix_sdk::deserialized_responses::TimelineEvent;
    use matrix_sdk::ruma::api::client::relations::get_relating_events_with_rel_type::v1;
    use matrix_sdk::ruma::events::relation::RelationType;
    use matrix_sdk::ruma::events::room::encrypted::OriginalSyncRoomEncryptedEvent;

    const MAX_THREAD_PAGES: usize = 20;
    const PAGE_SIZE: u32 = 100;

    let room_id = parse_room_id(room_id_raw)?;
    let root_event_id = parse_event_id(thread_root_event_id_raw)?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from(crate::helpers::ROOM_NOT_AVAILABLE))?;

    let mut from = None;
    let mut timeline_events = Vec::new();
    let mut seen_event_ids = std::collections::HashSet::new();

    for _ in 0..MAX_THREAD_PAGES {
        let mut request =
            v1::Request::new(room_id.clone(), root_event_id.clone(), RelationType::Thread);
        request.from = from.clone();
        request.limit = Some(PAGE_SIZE.into());
        let response = client
            .send(request)
            .await
            .map_err(|error| format!("Failed to load Matrix thread replies: {error}"))?;
        let next_batch = response.next_batch.clone();

        for raw_event in response.chunk {
            let event = raw_event
                .deserialize_as::<serde_json::Value>()
                .map_err(|error| format!("Failed to parse Matrix thread event: {error}"))?;
            let event_id = event
                .get("event_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if event_id.is_empty() || !seen_event_ids.insert(event_id.to_owned()) {
                continue;
            }

            let event_type = event
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let timeline_event = if event_type == "m.room.encrypted" {
                let encrypted_event: matrix_sdk::ruma::serde::Raw<OriginalSyncRoomEncryptedEvent> =
                    raw_event.cast_unchecked();
                room.decrypt_event(&encrypted_event, None)
                    .await
                    .map_err(|error| format!("Failed to decrypt Matrix thread reply: {error}"))?
            } else {
                TimelineEvent::from_plaintext(raw_event.cast_unchecked())
            };
            timeline_events.push(timeline_event);
        }

        let Some(next_batch) = next_batch else {
            break;
        };
        if from.as_deref() == Some(next_batch.as_str()) {
            break;
        }
        from = Some(next_batch);
    }

    let (mut messages, _) = parse_message_chunk(
        &DefaultMediaResolver,
        client,
        &room_id,
        timeline_events,
        &mut ReactionAccumulator::new(),
    )
    .await;
    messages.retain(|message| message.thread_root_id.as_deref() == Some(thread_root_event_id_raw));
    messages.sort_by_key(|message| message.timestamp.unwrap_or_default());
    Ok(messages)
}

/// Some homeservers do not include bundled `m.thread` counts on `/messages`
/// responses. Ask `/relations` for roots that have no bundled count so existing
/// threads remain discoverable in the timeline. Keep this bounded per page and
/// run the independent lookups concurrently.
pub(super) async fn hydrate_thread_summaries(
    client: &matrix_sdk::Client,
    room_id: &matrix_sdk::ruma::RoomId,
    messages: &mut [types::chat::MatrixChatMessage],
) {
    const MAX_ROOT_LOOKUPS_PER_PAGE: usize = 50;

    let roots: Vec<String> = messages
        .iter()
        .filter(|message| {
            message.thread_root_id.is_none()
                && message.thread_reply_count == 0
                && !message.is_deleted
                && message.message_type.is_some()
        })
        .filter_map(|message| message.event_id.clone())
        .take(MAX_ROOT_LOOKUPS_PER_PAGE)
        .collect();

    if roots.is_empty() {
        return;
    }

    let client = client.clone();
    let room_id = room_id.to_owned();
    let summaries = stream::iter(roots.into_iter().map(move |event_id| {
        let client = client.clone();
        let room_id = room_id.clone();
        async move {
            let summary = fetch_thread_summary(&client, &room_id, &event_id).await;
            (event_id, summary)
        }
    }))
    .buffer_unordered(12)
    .collect::<Vec<_>>()
    .await;

    for (event_id, summary) in summaries {
        let Some((reply_count, participants)) = summary else {
            continue;
        };
        if let Some(root) = messages
            .iter_mut()
            .find(|message| message.event_id.as_deref() == Some(event_id.as_str()))
        {
            root.thread_reply_count = root.thread_reply_count.max(reply_count);
            if root.thread_participants.is_empty() {
                root.thread_participants = participants;
            }
        }
    }
}

async fn fetch_thread_summary(
    client: &matrix_sdk::Client,
    room_id: &matrix_sdk::ruma::RoomId,
    root_event_id: &str,
) -> Option<(u32, Vec<String>)> {
    use matrix_sdk::ruma::api::client::relations::get_relating_events_with_rel_type::v1;
    use matrix_sdk::ruma::events::relation::RelationType;

    let root_event_id = parse_event_id(root_event_id).ok()?;
    let mut request = v1::Request::new(room_id.to_owned(), root_event_id, RelationType::Thread);
    request.limit = Some(100u32.into());
    let response = client.send(request).await.ok()?;

    let mut reply_count = 0u32;
    let mut participants = Vec::new();
    for raw_event in response.chunk {
        let Ok(event) = raw_event.deserialize_as::<serde_json::Value>() else {
            continue;
        };
        let sender = event
            .get("sender")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        reply_count = reply_count.saturating_add(1);
        if let Some(sender) = sender.filter(|sender| !participants.contains(sender)) {
            participants.push(sender);
        }
    }

    (reply_count > 0).then_some((reply_count, participants.into_iter().take(3).collect()))
}
