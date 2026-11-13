//! Per-session sync transport. Keep the Sliding Sync position across worker passes.
use futures_util::StreamExt;
use matrix_sdk::ruma::api::client::sync::sync_events::v5::request::RoomSubscription;
use matrix_sdk::ruma::{events::StateEventType, OwnedRoomId};
use matrix_sdk::sliding_sync::Version;
use matrix_sdk::{Client, SlidingSync, SlidingSyncList, SlidingSyncMode};
use std::time::Duration;

pub(crate) struct RoomSyncEngine {
    sliding: Option<SlidingSync>,
    selected: Option<OwnedRoomId>,
    first_response_pending: bool,
}

fn required_state() -> Vec<(StateEventType, String)> {
    vec![
        (StateEventType::RoomCreate, "".into()),
        (StateEventType::RoomCanonicalAlias, "".into()),
        (StateEventType::RoomTombstone, "".into()),
        (StateEventType::RoomName, "".into()),
        (StateEventType::RoomAvatar, "".into()),
        (StateEventType::RoomEncryption, "".into()),
        (StateEventType::SpaceChild, "*".into()),
        (StateEventType::RoomMember, "$LAZY".into()),
        (StateEventType::RoomMember, "$ME".into()),
    ]
}

impl RoomSyncEngine {
    pub(crate) async fn new(client: &Client) -> Result<Self, String> {
        client
            .event_cache()
            .subscribe()
            .map_err(|e| e.to_string())?;
        let sliding = if client
            .available_sliding_sync_versions()
            .await
            .iter()
            .any(|version| matches!(version, Version::Native))
        {
            let list = SlidingSyncList::builder("rooms")
                .sync_mode(SlidingSyncMode::new_growing(32))
                .timeline_limit(20)
                .required_state(required_state());
            let sync = client
                .sliding_sync("desktop")
                .map_err(|e| e.to_string())?
                .version(Version::Native)
                .poll_timeout(Duration::from_secs(25))
                .with_all_extensions()
                .add_cached_list(list)
                .await
                .map_err(|e| e.to_string())?
                .build()
                .await
                .map_err(|e| e.to_string())?;
            log::info!("Room sync transport: native Sliding Sync (MSC4186)");
            Some(sync)
        } else {
            log::info!("Room sync transport: classic sync (native Sliding Sync unavailable)");
            None
        };
        Ok(Self {
            sliding,
            selected: None,
            first_response_pending: true,
        })
    }

    pub(crate) async fn next(
        &mut self,
        client: &Client,
        selected: Option<&str>,
        timeout: Duration,
    ) -> Result<Vec<OwnedRoomId>, String> {
        if let Some(sync) = &self.sliding {
            let selected = selected.and_then(|id| id.parse::<OwnedRoomId>().ok());
            if selected != self.selected {
                if let Some(old) = &self.selected {
                    sync.remove_room_subscriptions(&[old.as_ref()], false);
                }
                if let Some(room) = &selected {
                    let mut subscription = RoomSubscription::default();
                    subscription.timeline_limit = 50u32.into();
                    subscription.required_state = required_state();
                    sync.add_room_subscriptions(&[room.as_ref()], Some(subscription), false);
                }
                self.selected = selected;
            }
            let started = std::time::Instant::now();
            let stream = sync.sync();
            futures_util::pin_mut!(stream);
            match stream.next().await {
                Some(Ok(update)) => {
                    if self.first_response_pending {
                        log::info!(
                            "Native Sliding Sync first response: {} rooms in {} ms",
                            update.rooms.len(),
                            started.elapsed().as_millis()
                        );
                        self.first_response_pending = false;
                    }
                    Ok(update.rooms)
                }
                Some(Err(error)) => Err(format!("Sliding Sync failed: {error}")),
                None => Err("Sliding Sync stream ended".into()),
            }
        } else {
            protocol::sync::sync_once_serialized(
                client,
                matrix_sdk::config::SyncSettings::default().timeout(timeout),
            )
            .await
            .map_err(|e| format!("Failed to sync Matrix rooms: {e}"))?;
            Ok(Vec::new())
        }
    }
}
