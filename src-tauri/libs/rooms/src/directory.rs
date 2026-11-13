use std::sync::Arc;

use auth::AuthState;
use matrix_sdk::ruma::{
    api::client::directory::get_public_rooms_filtered::v3::Request as PublicRoomsRequest,
    directory::Filter,
    ServerName,
};
use storage::AppDb;
use types::{rooms::{MatrixPublicRoom, MatrixSearchPublicRoomsRequest, MatrixSearchPublicRoomsResponse}, Paths};

pub async fn search_public_rooms(
    request: MatrixSearchPublicRoomsRequest,
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &Arc<AuthState>,
) -> Result<MatrixSearchPublicRoomsResponse, String> {
    let server_name = request.server_name.trim();
    let server = <&ServerName>::try_from(server_name)
        .map_err(|error| format!("Invalid directory server: {error}"))?
        .to_owned();
    let client = auth_state.restore_client_and_get(paths, app_db).await?;

    let mut query = PublicRoomsRequest::new();
    query.server = Some(server);
    query.limit = Some((request.limit.unwrap_or(20).clamp(1, 50)).into());
    query.since = request.since;
    let mut filter = Filter::new();
    let search_term = request.search_term.unwrap_or_default().trim().to_owned();
    if !search_term.is_empty() {
        filter.generic_search_term = Some(search_term);
    }
    query.filter = filter;

    let response = client.public_rooms_filtered(query).await
        .map_err(|error| format!("Failed to search {server_name}: {error}"))?;
    let directory_server = server_name.to_owned();
    // Don't fetch every room avatar before responding. A directory page may
    // contain many MXC URLs, and some media servers reject remote fetches.
    // Return the MXC URI so the UI can request each public avatar lazily.
    let rooms = response.chunk.into_iter().map(|room| MatrixPublicRoom {
        room_id: room.room_id.to_string(),
        name: room.name,
        canonical_alias: room.canonical_alias.map(|alias| alias.to_string()),
        avatar_url: room.avatar_url.map(|url| url.to_string()),
        topic: room.topic,
        joined_members: u64::from(room.num_joined_members),
        world_readable: room.world_readable,
        directory_server: directory_server.clone(),
    }).collect();

    Ok(MatrixSearchPublicRoomsResponse {
        rooms,
        next_batch: response.next_batch,
        total_room_count_estimate: response.total_room_count_estimate.map(u64::from),
    })
}
