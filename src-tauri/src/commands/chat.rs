//! Chat command adapters: message fetch/stream, send (text + media), reactions,
//! emoji packs, user avatars, and clipboard. Delegates to the `chat` crate.

use std::{fs, sync::Arc};

use tauri::State;

use auth::AuthState;
use storage::AppDb;
use types::chat::{
    MatrixCancelMediaTranscodeRequest, MatrixCancelMediaTranscodeResponse,
    MatrixChatMessageStreamEvent, MatrixCopyImageToClipboardRequest, MatrixDownloadFileRequest,
    MatrixDownloadFileResponse, MatrixGetChatMessagesRequest, MatrixGetChatMessagesResponse,
    MatrixGetEmojiPacksResponse, MatrixGetPinnedEventsRequest, MatrixGetPinnedEventsResponse,
    MatrixGetRoomMembersRequest, MatrixGetUploadLimitResponse,
    MatrixGetUserAvatarRequest, MatrixGetUserAvatarResponse, MatrixRedactMessageRequest,
    MatrixRedactMessageResponse, MatrixReplyEventRequest, MatrixResolveVideoUrlRequest,
    MatrixSetMessagePinnedRequest, MatrixSetMessagePinnedResponse,
    MatrixResolveVideoUrlResponse, MatrixRoomMember, MatrixSendChatMessageRequest,
    MatrixSendChatMessageResponse, MatrixSendMediaFileRequest, MatrixSendMediaFileResponse,
    MatrixStreamChatMessagesRequest, MatrixStreamChatMessagesResponse, MatrixToggleReactionRequest,
    MatrixToggleReactionResponse,
};
use types::{event_paths, Paths, RoomUpdateTriggerState};

#[tauri::command]
pub fn matrix_get_file_size(file_path: String) -> Result<u64, String> {
    fs::metadata(&file_path)
        .map(|metadata| metadata.len())
        .map_err(|error| format!("Failed to read file metadata: {error}"))
}

#[tauri::command]
pub fn matrix_save_pasted_file(file_name: String, bytes: Vec<u8>) -> Result<String, String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let extension = std::path::Path::new(&file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| {
            !extension.is_empty()
                && extension.len() <= 8
                && extension
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
        .unwrap_or("png");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Failed to create a temporary file name: {error}"))?
        .as_nanos();
    let path = std::env::temp_dir().join(format!("matrixgram-pasted-{unique}.{extension}"));
    fs::write(&path, bytes).map_err(|error| format!("Failed to save pasted file: {error}"))?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn matrix_remove_pasted_file(file_path: String) -> Result<(), String> {
    let temp_dir = std::env::temp_dir();
    let path = std::path::Path::new(&file_path);
    let is_app_temp_file = path.parent() == Some(temp_dir.as_path())
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("matrixgram-pasted-"));
    if is_app_temp_file && path.exists() {
        fs::remove_file(path).map_err(|error| format!("Failed to remove pasted file: {error}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn matrix_get_upload_limit(
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixGetUploadLimitResponse, String> {
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    let size = client
        .load_or_fetch_max_upload_size()
        .await
        .map_err(|error| format!("Failed to read the homeserver upload limit: {error}"))?;
    Ok(MatrixGetUploadLimitResponse {
        max_upload_size: u64::from(size),
    })
}

#[tauri::command]
pub async fn matrix_get_chat_messages(
    request: MatrixGetChatMessagesRequest,
    auth_state: State<'_, Arc<AuthState>>,
    app_db: State<'_, Arc<AppDb>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    paths: State<'_, Paths>,
) -> Result<MatrixGetChatMessagesResponse, String> {
    log::info!("matrix_get_chat_messages requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;

    let from = request.from.clone();
    let cacheable_initial_request =
        chat::persistence::is_cacheable_initial_request(from.as_deref(), request.limit);
    let limit = request.limit;

    if let Some(cached) = chat::persistence::load_initial_room_messages(
        &app_db,
        request.room_id.as_str(),
        from.as_deref(),
        limit,
    )
    .await?
    {
        let _ = room_update_trigger_state.enqueue_refresh(Some(request.room_id.clone()), true);

        if !chat::helpers::has_stale_cached_media_urls(&cached.messages) {
            return Ok(cached);
        }
    }

    let response = match chat::get_chat_messages(
        &client,
        request.room_id.as_str(),
        from.clone(),
        limit,
    )
    .await
    {
        Ok(response) => response,
        Err(error) if chat::helpers::is_room_unavailable_error(&error) => {
            protocol::sync::sync_once_brief(&client)
                .await
                .map_err(|sync_error| {
                    format!(
                        "Failed to sync Matrix room messages after room-unavailable error: {sync_error}"
                    )
                })?;

            chat::get_chat_messages(&client, request.room_id.as_str(), from, limit).await?
        }
        Err(error) => return Err(error),
    };

    let response = MatrixGetChatMessagesResponse {
        room_id: response.room_id,
        next_from: response.next_from,
        messages: response.messages.into_iter().rev().collect(),
    };

    if cacheable_initial_request {
        chat::persistence::store_initial_room_messages(&app_db, &response).await?;
    }

    Ok(response)
}

#[tauri::command]
pub async fn matrix_get_room_members(
    request: MatrixGetRoomMembersRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<Vec<MatrixRoomMember>, String> {
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    let room_id = protocol::parse_room_id(&request.room_id)?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| format!("Room {room_id} is not available in current session"))?;
    let members = room
        .members(matrix_sdk::RoomMemberships::JOIN)
        .await
        .map_err(|error| format!("Failed to load room members: {error}"))?;

    Ok(members
        .into_iter()
        .map(|member| MatrixRoomMember {
            user_id: member.user_id().to_string(),
            display_name: member
                .display_name()
                .unwrap_or_else(|| member.user_id().localpart())
                .to_owned(),
        })
        .collect())
}

#[tauri::command]
pub async fn matrix_stream_chat_messages(
    request: MatrixStreamChatMessagesRequest,
    auth_state: State<'_, Arc<AuthState>>,
    app_db: State<'_, Arc<AppDb>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    event_sink: State<'_, Arc<dyn types::EventSink>>,
    paths: State<'_, Paths>,
) -> Result<MatrixStreamChatMessagesResponse, String> {
    log::info!("matrix_stream_chat_messages requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;

    let app_db = app_db.inner().clone();
    let trigger_state = room_update_trigger_state.inner().clone();
    let event_sink = event_sink.inner().clone();
    let client_for_task = client.clone();
    let request_for_task = request.clone();
    if matches!(
        request.load_kind,
        types::chat::MatrixMessageLoadKind::Initial
    ) {
        let _ = trigger_state.enqueue_refresh(Some(request.room_id.clone()), false);
    }
    let terminal_room_id = request.room_id.clone();
    let terminal_stream_id = request.stream_id.clone();
    let terminal_load_kind = request.load_kind;
    // A stream that fails with an empty buffer never advanced its cursor, so
    // the terminal event must carry the cursor it was given rather than null:
    // the frontend overwrites `nextFrom` unconditionally, and nulling it here
    // would disable "load older" for the rest of the session after a single
    // transient failure. On success the stream emits its own terminal event
    // with the real cursor, which clears `nextFrom` when the room is exhausted.
    let terminal_next_from = request.from.clone();

    tauri::async_runtime::spawn(async move {
        let context = chat::receive::StreamRoomMessagesContext {
            event_sink: event_sink.as_ref(),
            app_db: &app_db,
            room_update_trigger_state: &trigger_state,
            client: &client_for_task,
        };

        let stream_result = chat::stream_chat_messages(context, request_for_task.clone()).await;

        let mut stream_failed = false;

        if let Err(error) = stream_result {
            if chat::helpers::is_room_unavailable_error(&error) {
                if let Err(sync_error) = protocol::sync::sync_once_brief(&client_for_task).await {
                    log::warn!(
                        "Background matrix stream sync failed after room-unavailable error: {sync_error}"
                    );
                    stream_failed = true;
                } else {
                    let context = chat::receive::StreamRoomMessagesContext {
                        event_sink: event_sink.as_ref(),
                        app_db: &app_db,
                        room_update_trigger_state: &trigger_state,
                        client: &client_for_task,
                    };
                    if let Err(retry_error) =
                        chat::stream_chat_messages(context, request_for_task).await
                    {
                        log::warn!("Background matrix stream retry failed: {retry_error}");
                        stream_failed = true;
                    }
                }
            } else {
                log::warn!("Background matrix stream failed: {error}");
                stream_failed = true;
            }
        }

        // The frontend only clears its loading state on a terminal `done`
        // event. If the stream failed without emitting one, emit a terminal
        // `done` (with no messages) so the UI never hangs in an infinite
        // loading loop.
        if stream_failed {
            match serde_json::to_value(MatrixChatMessageStreamEvent {
                room_id: terminal_room_id,
                stream_id: terminal_stream_id,
                load_kind: terminal_load_kind,
                sequence: 0,
                message: None,
                messages: None,
                next_from: terminal_next_from,
                done: true,
            }) {
                Ok(payload) => {
                    let _ = event_sink.emit(event_paths::CHAT_MESSAGES_STREAM, &payload);
                }
                Err(error) => {
                    log::warn!("Failed to serialize chat message stream completion: {error}");
                }
            }
        }
    });

    Ok(MatrixStreamChatMessagesResponse {
        stream_id: request.stream_id,
        started: true,
    })
}

#[tauri::command]
pub async fn matrix_get_emoji_packs(
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixGetEmojiPacksResponse, String> {
    log::info!("matrix_get_emoji_packs requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    chat::get_emoji_packs(&client).await
}

#[tauri::command]
pub async fn matrix_get_user_avatar(
    request: MatrixGetUserAvatarRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixGetUserAvatarResponse, String> {
    log::info!("matrix_get_user_avatar requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    chat::get_user_avatar(
        &client,
        &app_db,
        request.room_id.as_str(),
        request.user_id.as_str(),
    )
    .await
}

#[tauri::command]
pub async fn matrix_send_chat_message(
    request: MatrixSendChatMessageRequest,
    auth_state: State<'_, Arc<AuthState>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixSendChatMessageResponse, String> {
    log::info!("matrix_send_chat_message requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;

    // No sync on the happy path.
    //
    // The room worker keeps a long poll open and the SDK serialises
    // `sync_once` calls, so a sync here would queue behind that poll and delay
    // the send by up to a minute. The room state the send needs is already in
    // the SDK's cache.
    //
    // The one case that needs a sync is a room the SDK has not seen yet, for
    // instance right after signing in. That path is rare enough to be worth a
    // short sync and a retry.
    let room_id = request.room_id.clone();
    let picker_custom_emoji = chat::emoji::load_picker_assets_from_client(&client).await?;
    let response = match chat::send_chat_message(
        &client,
        room_id.as_str(),
        request.body.as_str(),
        &picker_custom_emoji,
        request.in_reply_to_event_id.as_deref(),
        request.formatted_body.as_deref(),
        &request.mentioned_user_ids,
    )
    .await
    {
        Ok(response) => Ok(response),
        Err(error) if chat::helpers::is_room_unavailable_error(&error) => {
            protocol::sync::sync_once_brief(&client)
                .await
                .map_err(|sync_error| format!("Failed to sync Matrix before send: {sync_error}"))?;

            chat::send_chat_message(
                &client,
                room_id.as_str(),
                request.body.as_str(),
                &picker_custom_emoji,
                request.in_reply_to_event_id.as_deref(),
                request.formatted_body.as_deref(),
                &request.mentioned_user_ids,
            )
            .await
        }
        Err(error) => Err(error),
    };

    let _ = room_update_trigger_state.enqueue_refresh(Some(room_id), response.is_err());

    response
}

#[tauri::command]
pub async fn matrix_send_media_file(
    request: MatrixSendMediaFileRequest,
    auth_state: State<'_, Arc<AuthState>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    media_transcode_cancellation_state: State<'_, chat::MediaTranscodeCancellationState>,
    event_sink: State<'_, Arc<dyn types::EventSink>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixSendMediaFileResponse, String> {
    log::info!("matrix_send_media_file requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;

    // No sync before the upload either: it would queue behind the room
    // worker's long poll (see `matrix_send_chat_message`).
    let room_id = request.room_id.clone();
    let response = chat::send_media_file(
        &client,
        &event_sink,
        media_transcode_cancellation_state.inner(),
        room_id.as_str(),
        request.file_path.as_str(),
        request.compress_media,
        request.caption.as_deref(),
    )
    .await;

    if response.is_ok() {
        let _ = matrix_remove_pasted_file(request.file_path.clone());
    }

    // A failed media send still refreshes, exactly as a failed text send does:
    // the room may have received an echo, or the local view may have shown
    // something the server never accepted.
    let _ = room_update_trigger_state.enqueue_refresh(Some(room_id), response.is_err());

    response
}

#[tauri::command]
pub async fn matrix_cancel_media_transcode(
    request: MatrixCancelMediaTranscodeRequest,
    media_transcode_cancellation_state: State<'_, chat::MediaTranscodeCancellationState>,
) -> Result<MatrixCancelMediaTranscodeResponse, String> {
    let cancelled = chat::cancel_media_transcode(
        media_transcode_cancellation_state.inner(),
        request.room_id.as_str(),
        request.file_path.as_str(),
    )
    .await;

    Ok(MatrixCancelMediaTranscodeResponse { cancelled })
}

#[tauri::command]
pub async fn matrix_toggle_reaction(
    request: MatrixToggleReactionRequest,
    auth_state: State<'_, Arc<AuthState>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixToggleReactionResponse, String> {
    log::info!("matrix_toggle_reaction requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;

    protocol::sync::sync_once_brief(&client)
        .await
        .map_err(|error| format!("Failed to sync Matrix before reaction toggle: {error}"))?;

    let room_id = request.room_id.clone();
    let response = chat::toggle_reaction(
        &client,
        room_id.as_str(),
        request.target_event_id.as_str(),
        request.key.as_str(),
    )
    .await?;

    let _ = room_update_trigger_state.enqueue_refresh(Some(room_id), true);

    Ok(response)
}

#[tauri::command]
pub async fn matrix_redact_message(
    request: MatrixRedactMessageRequest,
    auth_state: State<'_, Arc<AuthState>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixRedactMessageResponse, String> {
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    chat::redact_message_from_client(&client, &request.room_id, &request.event_id).await?;

    if let Err(error) = app_db
        .mark_message_redacted(&request.room_id, &request.event_id)
        .await
    {
        log::warn!(
            "Message was redacted on the server, but the local cache could not be updated: {error}"
        );
        if let Err(clear_error) = app_db.clear_room_message_cache(&request.room_id).await {
            log::error!("Could not clear the stale room cache after redaction: {clear_error}");
        }
    }
    let _ = room_update_trigger_state.enqueue_refresh(Some(request.room_id), true);
    Ok(MatrixRedactMessageResponse { redacted: true })
}

#[tauri::command]
pub async fn matrix_get_pinned_events(
    request: MatrixGetPinnedEventsRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixGetPinnedEventsResponse, String> {
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    let room_id = protocol::parse_room_id(&request.room_id)?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| format!("Room {room_id} is not available in current session"))?;
    let event_ids = room
        .load_pinned_events()
        .await
        .map_err(|error| format!("Failed to load pinned messages: {error}"))?
        .unwrap_or_default()
        .into_iter()
        .map(|event_id| event_id.to_string())
        .collect();
    Ok(MatrixGetPinnedEventsResponse { event_ids })
}

#[tauri::command]
pub async fn matrix_set_message_pinned(
    request: MatrixSetMessagePinnedRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
) -> Result<MatrixSetMessagePinnedResponse, String> {
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    let room_id = protocol::parse_room_id(&request.room_id)?;
    let event_id = protocol::parse_event_id(&request.event_id)?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| format!("Room {room_id} is not available in current session"))?;
    if request.pinned {
        room.pin_event(&event_id)
            .await
            .map_err(|error| format!("Failed to pin message: {error}"))?;
    } else {
        room.unpin_event(&event_id)
            .await
            .map_err(|error| format!("Failed to unpin message: {error}"))?;
    }
    let _ = room_update_trigger_state.enqueue_refresh(Some(request.room_id), true);
    Ok(MatrixSetMessagePinnedResponse { pinned: request.pinned })
}

#[tauri::command]
pub async fn matrix_copy_image_to_clipboard(
    request: MatrixCopyImageToClipboardRequest,
) -> Result<(), String> {
    log::info!("matrix_copy_image_to_clipboard requested");
    chat::copy_image_to_clipboard(request.image_url.as_str()).await
}

#[tauri::command]
pub async fn matrix_read_clipboard_text() -> Result<String, String> {
    log::info!("matrix_read_clipboard_text requested");
    chat::read_clipboard_text().await
}

#[tauri::command]
pub async fn matrix_resolve_video_url(
    request: MatrixResolveVideoUrlRequest,
    auth_state: State<'_, Arc<AuthState>>,
    app_db: State<'_, Arc<AppDb>>,
    paths: State<'_, Paths>,
    video_server: State<'_, Arc<assets::VideoServer>>,
) -> Result<MatrixResolveVideoUrlResponse, String> {
    log::info!("matrix_resolve_video_url requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    chat::resolve_video_url(
        &client,
        &video_server,
        request.room_id.as_str(),
        request.event_id.as_str(),
    )
    .await
}

#[tauri::command]
pub async fn matrix_download_file(
    request: MatrixDownloadFileRequest,
    app: tauri::AppHandle,
    auth_state: State<'_, Arc<AuthState>>,
    app_db: State<'_, Arc<AppDb>>,
    paths: State<'_, Paths>,
) -> Result<MatrixDownloadFileResponse, String> {
    log::info!("matrix_download_file requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;

    let Some(data) =
        chat::download_file_data(&client, request.room_id.as_str(), request.event_id.as_str())
            .await?
    else {
        return Ok(MatrixDownloadFileResponse { saved: false });
    };

    let default_name = data
        .file_name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            let extension = assets::file_extension_from_mime(
                data.mime_type
                    .as_deref()
                    .unwrap_or("application/octet-stream"),
            );
            format!("download.{extension}")
        });

    use tauri_plugin_dialog::DialogExt;
    let destination = app
        .dialog()
        .file()
        .set_title("Save file")
        .set_file_name(default_name)
        .blocking_save_file();

    let Some(destination) = destination else {
        // User cancelled the save dialog.
        return Ok(MatrixDownloadFileResponse { saved: false });
    };

    let destination = destination
        .into_path()
        .map_err(|error| format!("Failed to resolve save path: {error}"))?;

    assets::save_file_to_path(&data.bytes, &destination)?;

    Ok(MatrixDownloadFileResponse { saved: true })
}

#[tauri::command]
pub async fn matrix_get_user_profile(
    request: MatrixGetUserAvatarRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<types::chat::MatrixUserProfileResponse, String> {
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    chat::profiles::get_user_profile(&client, &request.room_id, &request.user_id).await
}

#[tauri::command]
pub async fn matrix_open_direct_chat(
    request: types::chat::MatrixOpenDirectChatRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
) -> Result<types::chat::MatrixOpenDirectChatResponse, String> {
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    let room_id = chat::profiles::open_direct_chat(&client, &request.user_id).await?;
    rooms::persistence::collect_and_store_chats(&app_db, &client).await;
    let _ = room_update_trigger_state.enqueue_refresh(Some(room_id.clone()), false);
    Ok(types::chat::MatrixOpenDirectChatResponse { room_id })
}

#[tauri::command]
pub async fn matrix_get_room_read_state(
    request: types::chat::MatrixRoomReadRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<types::chat::MatrixRoomReadState, String> {
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    chat::receive::read::state(&client, &request.room_id).await
}
#[tauri::command]
pub async fn matrix_mark_room_read(
    request: types::chat::MatrixMarkReadRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
) -> Result<(), String> {
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    chat::receive::read::mark(&client, &request).await?;
    let _ = room_update_trigger_state.enqueue_refresh(None, false);
    Ok(())
}
#[tauri::command]
pub async fn matrix_get_read_context(
    request: types::chat::MatrixReadContextRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
) -> Result<types::chat::MatrixReadContextResponse, String> {
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    let _ = room_update_trigger_state.enqueue_refresh(Some(request.room_id.clone()), false);
    chat::receive::read::context(&client, &app_db, &request.room_id, &request.event_id).await
}
#[tauri::command]
pub async fn matrix_get_reply_event(
    request: MatrixReplyEventRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<types::chat::MatrixChatMessage, String> {
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    chat::receive::read::reply_event(&client, &request.room_id, &request.event_id).await
}
#[tauri::command]
pub async fn matrix_get_newer_messages(
    request: MatrixGetChatMessagesRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<types::chat::MatrixReadContextResponse, String> {
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    chat::receive::read::newer(&client, &request).await
}
