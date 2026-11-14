use protocol::{parse_event_id, parse_room_id};

pub async fn redact_message_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    event_id_raw: &str,
) -> Result<(), String> {
    let room_id = parse_room_id(room_id_raw)?;
    let event_id = parse_event_id(event_id_raw)?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from("Room is not available in current session"))?;

    room.redact(&event_id, None, None)
        .await
        .map_err(|error| format!("Failed to delete message for everyone: {error}"))?;
    Ok(())
}
