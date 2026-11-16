use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use log::{error, info, warn};
use matrix_sdk::ruma::events::StateEventType;
use tokio::sync::mpsc;

use auth::handle_unknown_token_error;
use auth::AuthState;
use chat::fetch_room_messages_from_client;
use chat::store_initial_room_messages;
use storage::AppDb;
use types::chat::MatrixGetChatMessagesResponse;
use types::config;
use types::event_paths;
use types::rooms::{
    MatrixChatSummary, MatrixRoomKind, MatrixRoomRemovedEvent, MatrixSelectedRoomMessagesEvent,
};
use types::{EventSink, Paths, RoomRefreshTrigger, RoomUpdateTriggerState};

use crate::direct::direct_room_ids;
use crate::persistence::collect_and_store_chats;
use crate::sync::RoomSyncEngine;

pub type RoomSnapshot = HashMap<String, MatrixChatSummary>;

/// Preview text of the newest message in a room, with its sender for context.
///
/// Only rooms whose latest event is a plain `m.room.message` can be previewed
/// from the sync response: an encrypted room's latest event is the ciphertext.
/// The chat list falls back to the locally cached (decrypted) message for
/// those.
async fn last_message_preview(
    room: &matrix_sdk::Room,
    current_user_id: Option<&str>,
) -> Option<String> {
    let matrix_sdk::latest_events::LatestEventValue::Remote(timeline) = room.latest_event() else {
        return None;
    };

    let Ok(event) = timeline.raw().deserialize_as::<serde_json::Value>() else {
        return None;
    };

    let parsed = protocol::events_schema::parse_timeline_message(
        &event,
        types::chat::MatrixMessageDecryptionStatus::Plaintext,
        types::chat::MatrixMessageVerificationStatus::Unknown,
    )?;

    if parsed.encrypted {
        return None;
    }

    let sender_name = if current_user_id == Some(parsed.sender.as_str()) {
        String::from("You")
    } else if let Ok(user_id) = protocol::parse_user_id(&parsed.sender) {
        room.get_member_no_sync(&user_id)
            .await
            .ok()
            .flatten()
            .and_then(|member| member.display_name().map(ToOwned::to_owned))
            .unwrap_or_else(|| user_id.localpart().to_owned())
    } else {
        parsed.sender.clone()
    };

    let preview_body = parsed
        .formatted_body
        .as_deref()
        .map(protocol::sanitize::formatted_body_to_plain_text)
        .filter(|body| !body.is_empty())
        .unwrap_or(parsed.body);

    Some(format!("{sender_name}: {preview_body}"))
}

pub async fn collect_chat_summaries(client: &matrix_sdk::Client) -> Vec<MatrixChatSummary> {
    let joined_rooms = client.joined_rooms();
    let children_by_parent = children_room_ids_by_parent_room(&joined_rooms).await;
    let direct_room_ids = direct_room_ids(client).await;
    let current_user_id = client.user_id().map(ToString::to_string);
    let mut chats = Vec::with_capacity(joined_rooms.len());
    let mut seen_room_ids = HashSet::new();

    for room in &joined_rooms {
        let room_id_str = room.room_id().to_string();
        seen_room_ids.insert(room_id_str.clone());

        let display_name = room
            .display_name()
            .await
            .map(|name| name.to_string())
            .unwrap_or_else(|_| room.room_id().to_string());

        let encrypted = room
            .latest_encryption_state()
            .await
            .map(|state| state.is_encrypted())
            .unwrap_or(false);
        let joined_members = room.joined_members_count();
        let is_direct = direct_room_ids.contains(room.room_id().as_str());
        let kind = if room.is_space() {
            MatrixRoomKind::Space
        } else {
            MatrixRoomKind::Room
        };
        let children_room_ids = children_by_parent
            .get(room.room_id().as_str())
            .cloned()
            .unwrap_or_default();

        let last_message_preview = last_message_preview(room, current_user_id.as_deref()).await;
        let last_message_ts = room
            .latest_event_timestamp()
            // The SDK returns a `UInt` (an exact f64 integer), which converts
            // to `i64` rather than `u64`.
            .map(|timestamp| i64::from(timestamp.get()) as u64);
        let unread_count = chat::receive::read::unread_count(
            client,
            &room_id_str,
            room.num_unread_messages().min(u64::from(u32::MAX)) as u32,
            last_message_ts,
        )
        .await;
        let is_pinned = room
            .tags()
            .await
            .ok()
            .flatten()
            .map(|tags| tags.contains_key(&matrix_sdk::ruma::events::tag::TagName::Favorite))
            .unwrap_or(false);
        let is_muted = matches!(
            room.notification_mode().await,
            Some(matrix_sdk::notification_settings::RoomNotificationMode::Mute)
        );

        chats.push(MatrixChatSummary {
            last_message_preview,
            last_message_ts,
            unread_count,
            is_pinned,
            is_muted,
            room_id: room_id_str,
            display_name,
            image_url: None,
            encrypted,
            joined_members,
            kind,
            joined: true,
            is_invited: false,
            invite_via_servers: vec![],
            is_direct,
            children_room_ids,
        });
    }

    for child_ids in children_by_parent.values() {
        for child_id in child_ids {
            if seen_room_ids.contains(child_id) {
                continue;
            }
            seen_room_ids.insert(child_id.clone());

            let (display_name, encrypted, joined_members, kind, joined) =
                if let Ok(parsed_id) = <&matrix_sdk::ruma::RoomId>::try_from(child_id.as_str()) {
                    if let Some(room) = client.get_room(parsed_id) {
                        let display_name = room
                            .display_name()
                            .await
                            .map(|name| name.to_string())
                            .unwrap_or_else(|_| child_id.clone());
                        let encrypted = room
                            .latest_encryption_state()
                            .await
                            .map(|state| state.is_encrypted())
                            .unwrap_or(false);
                        let joined_members = room.joined_members_count();
                        let kind = if room.is_space() {
                            MatrixRoomKind::Space
                        } else {
                            MatrixRoomKind::Room
                        };
                        let joined = room.state() == matrix_sdk::RoomState::Joined;
                        (display_name, encrypted, joined_members, kind, joined)
                    } else {
                        (child_id.clone(), false, 0, MatrixRoomKind::Room, false)
                    }
                } else {
                    (child_id.clone(), false, 0, MatrixRoomKind::Room, false)
                };

            let children_room_ids = children_by_parent
                .get(child_id.as_str())
                .cloned()
                .unwrap_or_default();

            chats.push(MatrixChatSummary {
                last_message_preview: None,
                last_message_ts: None,
                unread_count: 0,
                is_pinned: false,
                is_muted: false,
                room_id: child_id.clone(),
                display_name,
                image_url: None,
                encrypted,
                joined_members,
                kind,
                joined,
                is_invited: false,
                invite_via_servers: vec![],
                is_direct: direct_room_ids.contains(child_id.as_str()),
                children_room_ids,
            });
        }
    }

    for room in client.invited_rooms() {
        let room_id = room.room_id().to_string();
        if seen_room_ids.contains(&room_id) {
            continue;
        }
        let display_name = room
            .display_name()
            .await
            .map(|name| name.to_string())
            .unwrap_or_else(|_| room_id.clone());
        let invite_via_servers = room
            .invite_details()
            .await
            .ok()
            .map(|details| vec![details.inviter_id.server_name().to_string()])
            .unwrap_or_default();
        chats.push(MatrixChatSummary {
            last_message_preview: None,
            last_message_ts: None,
            unread_count: 0,
            is_pinned: false,
            is_muted: false,
            room_id: room_id.clone(),
            display_name,
            image_url: None,
            encrypted: room
                .latest_encryption_state()
                .await
                .map(|state| state.is_encrypted())
                .unwrap_or(false),
            joined_members: room.joined_members_count(),
            kind: if room.is_space() {
                MatrixRoomKind::Space
            } else {
                MatrixRoomKind::Room
            },
            joined: false,
            is_invited: true,
            invite_via_servers,
            is_direct: direct_room_ids.contains(room_id.as_str()),
            children_room_ids: vec![],
        });
    }

    chats.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });

    chats
}

async fn children_room_ids_by_parent_room(
    joined_rooms: &[matrix_sdk::room::Room],
) -> HashMap<String, Vec<String>> {
    let mut children_by_parent = HashMap::<String, HashSet<String>>::new();

    for room in joined_rooms {
        if !room.is_space() {
            continue;
        }

        let parent_space_id = room.room_id().to_string();
        let state_events = match room
            .get_state_events(StateEventType::from("m.space.child"))
            .await
        {
            Ok(state_events) => state_events,
            Err(_) => continue,
        };

        for raw_event in state_events {
            let Ok(event) = serde_json::to_value(&raw_event) else {
                continue;
            };

            let Some(child_room_id) = event.get("state_key").and_then(|value| value.as_str())
            else {
                continue;
            };

            if child_room_id.is_empty() {
                continue;
            }

            let is_active_child = event
                .get("content")
                .and_then(|v| v.as_object())
                .map(|content| !content.is_empty())
                .unwrap_or(false);

            if !is_active_child {
                continue;
            }

            children_by_parent
                .entry(parent_space_id.clone())
                .or_default()
                .insert(child_room_id.to_string());
        }
    }

    children_by_parent
        .into_iter()
        .map(|(parent_room_id, child_ids)| {
            let mut child_ids = child_ids.into_iter().collect::<Vec<_>>();
            child_ids.sort();
            (parent_room_id, child_ids)
        })
        .collect()
}

/// The background room-update worker. Holds the receiver side of the trigger
/// channel plus the shared state needed to run a refresh pass.
///
/// This crate is Tauri-free and cannot assume a Tokio runtime is running, so it
/// does not spawn the loop itself. The binder spawns [`RoomUpdateWorker::run`]
/// on its async runtime (e.g. Tauri's managed runtime).
pub struct RoomUpdateWorker {
    receiver: mpsc::UnboundedReceiver<RoomRefreshTrigger>,
    paths: Paths,
    app_db: Arc<AppDb>,
    auth_state: Arc<AuthState>,
    event_sink: Arc<dyn EventSink>,
}

impl RoomUpdateWorker {
    /// Run the worker loop to completion. Spawn this on an async runtime.
    pub async fn run(mut self) {
        let initial_sync_timeout = Duration::ZERO;
        let long_poll_sync_timeout = Duration::from_secs(config::LONG_POLL_SYNC_TIMEOUT_SECONDS);
        let unauthenticated_delay = Duration::from_secs(config::WORKER_UNAUTH_SLEEP_SECONDS);
        let retry_initial_delay = Duration::from_millis(config::WORKER_RETRY_INITIAL_DELAY_MS);
        let startup_retry_max_delay =
            Duration::from_millis(config::WORKER_STARTUP_RETRY_MAX_DELAY_MS);
        let retry_max_delay = Duration::from_millis(config::WORKER_RETRY_MAX_DELAY_MS);

        let mut previous_snapshot = RoomSnapshot::new();
        let mut previous_generation = 0;
        let mut sync_engine = None;
        let mut selected_room_id = None::<String>;
        let mut include_selected_messages = false;
        let mut retry_delay = None::<Duration>;
        let mut pending_trigger = None::<RoomRefreshTrigger>;

        loop {
            // A room subscription belongs to one account/session. Apply new
            // UI triggers only after discarding the previous session's selection.
            if previous_generation != self.auth_state.generation() {
                selected_room_id = None;
                include_selected_messages = false;
            }
            if let Some(trigger) = pending_trigger.take() {
                apply_trigger(
                    trigger,
                    &mut selected_room_id,
                    &mut include_selected_messages,
                );
            }

            while let Ok(trigger) = self.receiver.try_recv() {
                apply_trigger(
                    trigger,
                    &mut selected_room_id,
                    &mut include_selected_messages,
                );
            }

            // A refresh pass ends in a long poll that holds the per-homeserver
            // sync lock for as long as the server keeps the request open, and
            // every send and reaction queues behind that lock. A new trigger
            // means the answer has already changed, so the in-flight poll is
            // cancelled rather than waited out: the next pass re-syncs with the
            // shorter initial timeout and picks up the new state.
            //
            // Read before the pass borrows the snapshot.
            let had_snapshot = !previous_snapshot.is_empty();

            let interrupted = {
                // The pass borrows the loop's locals rather than `self`, because a
                // pinned future keeps its borrows alive for the whole `select` and
                // would otherwise freeze the receiver too. The block ends before
                // the snapshot is touched again, releasing that borrow.
                let deps = RefreshDeps {
                    app_db: &self.app_db,
                    auth_state: &self.auth_state,
                    event_sink: &self.event_sink,
                    paths: &self.paths,
                };
                let receiver = &mut self.receiver;
                let pass = run_refresh_pass(
                    &deps,
                    &mut previous_snapshot,
                    &mut previous_generation,
                    &mut sync_engine,
                    RefreshOptions {
                        selected_room_id: selected_room_id.clone(),
                        include_selected_messages,
                        initial_sync_timeout,
                        long_poll_sync_timeout,
                    },
                );
                tokio::pin!(pass);

                tokio::select! {
                    result = &mut pass => match result {
                        Ok(refresh_completed) => {
                            include_selected_messages = false;
                            retry_delay = None;

                            if !refresh_completed
                                && !drain_triggers_and_wait(
                                    receiver,
                                    unauthenticated_delay,
                                    &mut selected_room_id,
                                    &mut include_selected_messages,
                                )
                                .await
                            {
                                break;
                            }

                            false
                        }
                        Err(error) => {
                            include_selected_messages = false;
                            if had_snapshot {
                                error!("Room update pass failed: {error}");
                            } else if is_transient_sync_timeout_error(&error) {
                                info!("Initial room sync timed out; retrying: {error}");
                            } else {
                                warn!("Initial room sync pass failed: {error}");
                            }

                            let max_retry_delay = if had_snapshot {
                                retry_max_delay
                            } else {
                                startup_retry_max_delay
                            };

                            let next_delay = retry_delay
                                .unwrap_or(retry_initial_delay)
                                .min(max_retry_delay);

                            retry_delay = Some(next_delay.saturating_mul(2).min(max_retry_delay));

                            if !drain_triggers_and_wait(
                                receiver,
                                next_delay,
                                &mut selected_room_id,
                                &mut include_selected_messages,
                            )
                            .await
                            {
                                break;
                            }

                            false
                        }
                    },
                    trigger = receiver.recv() => {
                        match trigger {
                            Some(trigger) => pending_trigger = Some(trigger),
                            None => break,
                        }

                        true
                    }
                }
            };

            // Room upserts are idempotent. Replay after interruption rather
            // than seeding a snapshot whose rooms the frontend may never have seen.
            if interrupted {
                previous_snapshot.clear();
            }
        }
    }
}

/// The shared state a refresh pass reads. Split out of [`RoomUpdateWorker`] so
/// the pass's future borrows nothing the loop needs to mutate.
struct RefreshDeps<'a> {
    app_db: &'a Arc<AppDb>,
    auth_state: &'a Arc<AuthState>,
    event_sink: &'a Arc<dyn EventSink>,
    paths: &'a Paths,
}

struct RefreshOptions {
    selected_room_id: Option<String>,
    include_selected_messages: bool,
    initial_sync_timeout: Duration,
    long_poll_sync_timeout: Duration,
}

/// Create the trigger channel and the worker. The binder manages the returned
/// [`RoomUpdateTriggerState`] and spawns [`RoomUpdateWorker::run`] on its async
/// runtime.
pub fn start_room_update_worker(
    paths: &Paths,
    app_db: Arc<AppDb>,
    auth_state: Arc<AuthState>,
    event_sink: Arc<dyn EventSink>,
) -> (RoomUpdateTriggerState, RoomUpdateWorker) {
    let (sender, receiver) = mpsc::unbounded_channel::<RoomRefreshTrigger>();
    let worker = RoomUpdateWorker {
        receiver,
        paths: paths.clone(),
        app_db,
        auth_state,
        event_sink,
    };
    (RoomUpdateTriggerState::new(sender), worker)
}

async fn run_refresh_pass(
    worker: &RefreshDeps<'_>,
    previous_snapshot: &mut RoomSnapshot,
    previous_generation: &mut u64,
    sync_engine: &mut Option<RoomSyncEngine>,
    options: RefreshOptions,
) -> Result<bool, String> {
    let paths = &worker.paths;
    let app_db = &worker.app_db;
    let auth_state = &worker.auth_state;

    auth_state
        .restore_client_from_disk_if_needed(paths, app_db)
        .await?;

    let client = match auth_state.client() {
        Ok(client) => client,
        Err(_) => {
            previous_snapshot.clear();
            *sync_engine = None;
            return Ok(false);
        }
    };
    let generation = auth_state.generation();
    if *previous_generation != generation {
        *sync_engine = None;
    }
    reset_snapshot_for_session(previous_snapshot, previous_generation, generation);
    let mut cancelled = auth_state.cancellation_receiver();
    let refresh = refresh_client_rooms(
        worker,
        previous_snapshot,
        &client,
        generation,
        sync_engine,
        options,
    );
    let result = tokio::select! {
        result = refresh => result,
        _ = async {
            match &mut cancelled {
                Some(receiver) => { let _ = receiver.wait_for(|stop| *stop).await; },
                None => std::future::pending::<()>().await,
            }
        } => Ok(false),
    };
    if matches!(&result, Ok(false)) {
        *sync_engine = None;
    }
    if let Err(error) = &result {
        if is_unknown_token_error(error) {
            // Clear auth outside the cancellation select: clearing auth also
            // fires that signal and must not cancel its own cleanup halfway.
            *sync_engine = None;
            handle_unknown_token_error(paths, app_db, auth_state, &client).await?;
            previous_snapshot.clear();
            return Ok(false);
        }
    }
    result
}

fn reset_snapshot_for_session(snapshot: &mut RoomSnapshot, previous: &mut u64, current: u64) {
    if *previous != current {
        snapshot.clear();
        *previous = current;
    }
}

fn emit_sync_status(sink: &Arc<dyn EventSink>, client: &matrix_sdk::Client, error: Option<String>) {
    if let Some(user_id) = client.user_id() {
        if let Ok(payload) = serde_json::to_value(types::rooms::MatrixRoomSyncStatusEvent {
            user_id: user_id.to_string(),
            error,
        }) {
            let _ = sink.emit(event_paths::ROOM_SYNC_STATUS, &payload);
        }
    }
}

async fn refresh_client_rooms(
    worker: &RefreshDeps<'_>,
    previous_snapshot: &mut RoomSnapshot,
    client: &matrix_sdk::Client,
    generation: u64,
    sync_engine: &mut Option<RoomSyncEngine>,
    options: RefreshOptions,
) -> Result<bool, String> {
    let RefreshOptions {
        selected_room_id,
        include_selected_messages,
        initial_sync_timeout,
        long_poll_sync_timeout,
    } = options;
    let app_db = &worker.app_db;
    let auth_state = &worker.auth_state;
    let event_sink = &worker.event_sink;
    if previous_snapshot.is_empty() {
        let local_chats = collect_and_store_chats(app_db, client).await;
        if auth_state.generation() != generation {
            return Ok(false);
        }
        if !local_chats.is_empty() {
            let mut local_snapshot = RoomSnapshot::new();
            for chat in local_chats {
                local_snapshot.insert(chat.room_id.clone(), chat);
            }

            for chat in local_snapshot.values() {
                let payload = serde_json::to_value(chat)
                    .map_err(|error| format!("Failed to serialize room: {error}"))?;
                let _ = event_sink.emit(event_paths::ROOM_ADDED, &payload);
            }

            *previous_snapshot = local_snapshot;
        }
    }

    let sync_timeout = if previous_snapshot.is_empty() {
        initial_sync_timeout
    } else {
        long_poll_sync_timeout
    };

    if sync_engine.is_none() {
        *sync_engine = Some(RoomSyncEngine::new(client).await?);
    }
    let changed_rooms = match sync_engine
        .as_mut()
        .unwrap()
        .next(client, selected_room_id.as_deref(), sync_timeout)
        .await
    {
        Ok(rooms) => rooms,
        Err(error) => {
            emit_sync_status(event_sink, client, Some(error.clone()));
            // A failed native Sliding Sync response (for example a transient
            // proxy 502 with a non-Matrix body) can leave its stream unusable.
            // Rebuild it on the next retry while keeping the room snapshot.
            *sync_engine = None;
            return Err(error);
        }
    };
    let current_snapshot: RoomSnapshot = collect_and_store_chats(app_db, client)
        .await
        .into_iter()
        .map(|chat| (chat.room_id.clone(), chat))
        .collect();

    if auth_state.generation() != generation {
        previous_snapshot.clear();
        return Ok(false);
    }

    // Whether the room that is currently open changed during this pass. A new
    // message changes the room summary (last message, unread count), so this
    // is how the open timeline learns about messages sent by another client.
    let mut selected_room_changed = changed_rooms
        .iter()
        .any(|id| Some(id.as_str()) == selected_room_id.as_deref());

    for (room_id, chat) in &current_snapshot {
        match previous_snapshot.get(room_id) {
            None => {
                let payload = serde_json::to_value(chat)
                    .map_err(|error| format!("Failed to serialize room: {error}"))?;
                let _ = event_sink.emit(event_paths::ROOM_ADDED, &payload);

                if selected_room_id.as_deref() == Some(room_id.as_str()) {
                    selected_room_changed = true;
                }
            }
            Some(previous) if previous != chat => {
                let payload = serde_json::to_value(chat)
                    .map_err(|error| format!("Failed to serialize room: {error}"))?;
                let _ = event_sink.emit(event_paths::ROOM_UPDATED, &payload);

                if selected_room_id.as_deref() == Some(room_id.as_str()) {
                    selected_room_changed = true;
                }
            }
            Some(_) => {}
        }
    }

    for room_id in previous_snapshot.keys() {
        if !current_snapshot.contains_key(room_id) {
            let payload = serde_json::to_value(MatrixRoomRemovedEvent {
                room_id: room_id.clone(),
            })
            .map_err(|error| format!("Failed to serialize room-removed event: {error}"))?;
            let _ = event_sink.emit(event_paths::ROOM_REMOVED, &payload);
        }
    }

    if include_selected_messages || selected_room_changed {
        if let Some(room_id) = selected_room_id {
            if current_snapshot.contains_key(&room_id) {
                if let Some(preview) =
                    chat::receive::cached_room_messages(client, &room_id, 50).await
                {
                    let payload = serde_json::to_value(MatrixSelectedRoomMessagesEvent {
                        room_id: preview.room_id,
                        next_from: preview.next_from,
                        messages: preview.messages,
                    })
                    .map_err(|error| error.to_string())?;
                    let _ = event_sink.emit(event_paths::SELECTED_ROOM_MESSAGES, &payload);
                }
                if let Ok(response) =
                    fetch_room_messages_from_client(client, &room_id, None, Some(50)).await
                {
                    let response = MatrixGetChatMessagesResponse {
                        room_id: response.room_id,
                        next_from: response.next_from,
                        messages: response.messages.into_iter().rev().collect(),
                    };

                    if let Err(error) = store_initial_room_messages(app_db, &response).await {
                        warn!("Failed to persist selected-room message cache: {error}");
                    }

                    let payload = serde_json::to_value(MatrixSelectedRoomMessagesEvent {
                        room_id: response.room_id,
                        next_from: response.next_from,
                        messages: response.messages,
                    })
                    .map_err(|error| {
                        format!("Failed to serialize selected-room messages: {error}")
                    })?;
                    let _ = event_sink.emit(event_paths::SELECTED_ROOM_MESSAGES, &payload);
                }
            }
        }
    }

    *previous_snapshot = current_snapshot;
    emit_sync_status(event_sink, client, None);
    Ok(true)
}

fn is_unknown_token_error(error: &str) -> bool {
    error.contains("M_UNKNOWN_TOKEN")
        || error.contains("refresh token does not exist")
        || error.contains("refresh token isn't valid anymore")
}

fn is_transient_sync_timeout_error(error: &str) -> bool {
    error.contains("error sending request")
        || error.contains("timed out")
        || error.contains("deadline has elapsed")
}

fn apply_trigger(
    trigger: RoomRefreshTrigger,
    selected: &mut Option<String>,
    include_selected_messages: &mut bool,
) {
    if let Some(room_id) = trigger.selected_room_id {
        *selected = if room_id.is_empty() {
            None
        } else {
            Some(room_id)
        };
    }

    if trigger.include_selected_messages {
        *include_selected_messages = true;
    }
}

/// Wait for a trigger or a delay, applying any received trigger to the worker
/// state. Returns `false` when the channel closed (worker should stop).
async fn drain_triggers_and_wait(
    receiver: &mut mpsc::UnboundedReceiver<RoomRefreshTrigger>,
    delay: Duration,
    selected_room_id: &mut Option<String>,
    include_selected_messages: &mut bool,
) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => true,
        maybe_trigger = receiver.recv() => {
            let Some(trigger) = maybe_trigger else {
                return false;
            };
            apply_trigger(trigger, selected_room_id, include_selected_messages);
            true
        }
    }
}

#[cfg(test)]
mod session_tests {
    use super::*;
    #[test]
    fn a_new_session_replays_even_unchanged_rooms() {
        let mut snapshot = RoomSnapshot::new();
        let room: MatrixChatSummary = serde_json::from_value(serde_json::json!({
            "roomId": "!room:example.org", "displayName": "Chat", "imageUrl": null,
            "encrypted": false, "joinedMembers": 2, "kind": "room", "joined": true,
            "isDirect": false, "childrenRoomIds": [], "lastMessagePreview": null,
            "lastMessageTs": null, "unreadCount": 0, "isPinned": false, "isMuted": false
        }))
        .unwrap();
        snapshot.insert(room.room_id.clone(), room);
        let mut generation = 1;
        reset_snapshot_for_session(&mut snapshot, &mut generation, 1);
        assert_eq!(snapshot.len(), 1);
        reset_snapshot_for_session(&mut snapshot, &mut generation, 3);
        assert!(snapshot.is_empty());
        assert_eq!(generation, 3);
    }
}
