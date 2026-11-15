use matrix_sdk::Client;
use types::chat::MatrixUserProfileResponse;

pub async fn get_user_profile(
    client: &Client,
    room_id: &str,
    user_id: &str,
) -> Result<MatrixUserProfileResponse, String> {
    let user = protocol::parse_user_id(user_id)?;
    let room_id = protocol::parse_room_id(room_id)?;
    let member = match client.get_room(&room_id) {
        Some(room) => room.get_member_no_sync(&user).await.ok().flatten(),
        None => None,
    };
    // Room membership is already synced. Never wait for a profile request or
    // image download merely to label a reaction popup.
    if let Some(member) = &member {
        return Ok(MatrixUserProfileResponse {
            user_id: user.to_string(),
            server_name: user.server_name().to_string(),
            display_name: member.display_name().map(ToOwned::to_owned),
            avatar_url: member
                .avatar_url()
                .and_then(|url| assets::image::cached_media_path_for_source_url(url.as_str())),
        });
    }
    let profile = client.account().fetch_user_profile_of(&user).await;
    let (display_name, avatar) = match profile {
        Ok(profile) => (
            profile
                .get("displayname")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned),
            profile
                .get("avatar_url")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned),
        ),
        Err(error) => match member {
            Some(member) => (
                member.display_name().map(ToOwned::to_owned),
                member.avatar_url().map(ToString::to_string),
            ),
            None => return Err(format!("Failed to load user profile: {error}")),
        },
    };
    let avatar_url = match avatar {
        Some(url) => crate::media::cache_mxc_media_to_local_path(client, &url).await,
        None => None,
    };
    Ok(MatrixUserProfileResponse {
        user_id: user.to_string(),
        server_name: user.server_name().to_string(),
        display_name,
        avatar_url,
    })
}

pub async fn open_direct_chat(
    client: &Client,
    user_id: &str,
    encrypted: bool,
) -> Result<String, String> {
    let user = protocol::parse_user_id(user_id)?;
    // Serialize lookup + create so repeated clicks cannot create duplicate DMs.
    static DM_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = DM_LOCK.lock().await;
    let room = match client.get_dm_room(&user) {
        Some(room) => room,
        None => {
            use matrix_sdk::ruma::api::client::room::create_room::v3::{Request, RoomPreset};
            use matrix_sdk::ruma::events::{
                room::encryption::RoomEncryptionEventContent, InitialStateEvent,
            };

            let mut request = Request::new();
            request.invite = vec![user];
            request.is_direct = true;
            request.preset = Some(RoomPreset::TrustedPrivateChat);
            if encrypted {
                request.initial_state = vec![InitialStateEvent::with_empty_state_key(
                    RoomEncryptionEventContent::with_recommended_defaults(),
                )
                .to_raw_any()];
            }

            client
                .create_room(request)
                .await
                .map_err(|error| format!("Failed to start direct chat: {error}"))?
        }
    };
    Ok(room.room_id().to_string())
}
