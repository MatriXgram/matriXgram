//! Forward incoming reactions to the UI after verifying the target belongs to us.
use std::sync::Arc;

use matrix_sdk::ruma::events::{
    reaction::OriginalSyncReactionEvent, room::redaction::OriginalSyncRoomRedactionEvent,
};
use matrix_sdk::{room::Room, Client};
use serde_json::{json, Value};
use types::{event_paths, EventSink};

fn is_own_visible_message(event: &Value, user_id: &str) -> bool {
    event.get("sender").and_then(Value::as_str) == Some(user_id)
        && matches!(
            event.get("type").and_then(Value::as_str),
            Some("m.room.message" | "m.sticker")
        )
        && event
            .get("unsigned")
            .and_then(|unsigned| unsigned.get("redacted_because"))
            .is_none()
        && event
            .get("content")
            .and_then(Value::as_object)
            .is_some_and(|content| !content.is_empty())
        && event
            .pointer("/content/m.relates_to/rel_type")
            .and_then(Value::as_str)
            != Some("m.replace")
}

pub fn watch_reactions(
    client: &Client,
    sink: Arc<dyn EventSink>,
    cancelled: tokio::sync::watch::Receiver<bool>,
) {
    let handle = client.add_event_handler({
        let sink = sink.clone();
        let cancelled = cancelled.clone();
        move |event: OriginalSyncReactionEvent, room: Room, client: Client| {
            let sink = sink.clone();
            let mut cancelled = cancelled.clone();
            async move {
                let Some(user_id) = client.user_id().map(ToOwned::to_owned) else { return };
                if event.sender == user_id { return; }
                tokio::spawn(async move {
                    tokio::select! {
                        _ = cancelled.wait_for(|stop| *stop) => {},
                        _ = async {
                            let target_id = &event.content.relates_to.event_id;
                            let target = match room.load_or_fetch_event(target_id, None).await {
                                Ok(target) => target,
                                Err(error) => { log::debug!("Unable to inspect reaction target: {error}"); return; }
                            };
                            let Ok(raw) = target.raw().deserialize_as::<serde_json::Value>() else { return };
                            if !is_own_visible_message(&raw, user_id.as_str()) {
                                return;
                            }
                            let sender_name = room.get_member_no_sync(&event.sender).await.ok().flatten()
                                .and_then(|member| member.display_name().map(ToOwned::to_owned))
                                .unwrap_or_else(|| event.sender.localpart().to_owned());
                            let payload = json!({
                                "userId": user_id.as_str(), "roomId": room.room_id().as_str(),
                                "eventId": event.event_id.as_str(), "targetEventId": target_id.as_str(),
                                "sender": event.sender.as_str(), "senderName": sender_name,
                                "key": event.content.relates_to.key,
                                "timestamp": u64::from(event.origin_server_ts.get()),
                            });
                            let _ = sink.emit(event_paths::REACTION_RECEIVED, &payload);
                        } => {},
                    }
                });
            }
        }
    });
    let redaction_handle = client.add_event_handler(move |event: OriginalSyncRoomRedactionEvent, room: Room, client: Client| {
        let sink = sink.clone();
        let cancelled = cancelled.clone();
        async move {
            if *cancelled.borrow() { return; }
            let Some(user_id) = client.user_id() else { return };
            let Some(redacts) = event.content.redacts.as_ref().or(event.redacts.as_ref()) else { return };
            let _ = sink.emit(event_paths::REACTION_REDACTED, &json!({
                "userId": user_id.as_str(), "roomId": room.room_id().as_str(), "eventId": redacts.as_str(),
            }));
        }
    });
    std::mem::forget(handle);
    std::mem::forget(redaction_handle);
}

#[cfg(test)]
mod tests {
    use super::{is_own_visible_message, json};

    #[test]
    fn only_live_messages_from_the_current_user_are_targets() {
        let user = "@me:example.org";
        let own = json!({"type": "m.room.message", "sender": user, "content": {"body": "hello"}});
        assert!(is_own_visible_message(&own, user));
        assert!(!is_own_visible_message(&own, "@other:example.org"));
        assert!(!is_own_visible_message(
            &json!({"type": "m.room.message", "sender": user, "content": {}}),
            user
        ));
        assert!(!is_own_visible_message(
            &json!({"type": "m.room.message", "sender": user, "content": {"body": "hello"}, "unsigned": {"redacted_because": {}}}),
            user
        ));
        assert!(!is_own_visible_message(
            &json!({"type": "m.room.message", "sender": user, "content": {"m.relates_to": {"rel_type": "m.replace"}}}),
            user
        ));
        assert!(!is_own_visible_message(
            &json!({"type": "m.reaction", "sender": user, "content": {"body": "hello"}}),
            user
        ));
        assert!(is_own_visible_message(
            &json!({"type": "m.sticker", "sender": user, "content": {"url": "mxc://example.org/id"}}),
            user
        ));
    }
}
