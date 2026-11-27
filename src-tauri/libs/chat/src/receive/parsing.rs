use std::collections::{BTreeMap, HashMap};

use matrix_sdk::deserialized_responses::{TimelineEvent, VerificationState};
use serde_json::Value;

use protocol::events_schema::{
    extract_html_attribute, parse_reaction_event, parse_timeline_message,
};

use crate::media::MediaResolver;
use types::chat::{
    MatrixChatMessage, MatrixCustomEmoji, MatrixMessageDecryptionStatus,
    MatrixMessageVerificationStatus, MatrixReactionSummary,
};
use types::event_types;

/// Resolve the display name of a sender from the room member list.
///
/// Falls back to the localpart of the MXID when the member is unknown, which
/// is what a chat list needs anyway: the raw `@alice:example.org` is never a
/// useful label.
async fn resolve_sender_display_name(
    client: &matrix_sdk::Client,
    room_id: &matrix_sdk::ruma::RoomId,
    sender: &str,
) -> Option<String> {
    let Ok(user_id) = protocol::parse_user_id(sender) else {
        return None;
    };

    let Some(room) = client.get_room(room_id) else {
        return Some(user_id.localpart().to_string());
    };

    match room.get_member_no_sync(&user_id).await {
        Ok(Some(member)) => Some(
            member
                .display_name()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| user_id.localpart().to_string()),
        ),
        _ => Some(user_id.localpart().to_string()),
    }
}

/// Reactions seen so far, keyed by the event they annotate, then by reaction
/// key, then by sender.
///
/// This has to outlive a single network page: backward pagination returns
/// newest->oldest, so a reaction is always authored *after* its target and
/// therefore arrives in an *earlier* page than the message it decorates.
pub(super) type ReactionAccumulator =
    BTreeMap<String, BTreeMap<String, BTreeMap<String, Option<u64>>>>;

/// Attach the accumulated reactions to the messages of a freshly parsed page.
///
/// Entries whose target has not been seen yet are deliberately retained, since
/// the target is usually still waiting in an older page.
pub(super) fn apply_reactions(messages: &mut [MatrixChatMessage], reactions: &ReactionAccumulator) {
    if reactions.is_empty() {
        return;
    }

    for message in messages.iter_mut() {
        let Some(event_id) = &message.event_id else {
            continue;
        };

        let Some(reaction_map) = reactions.get(event_id) else {
            continue;
        };

        message.reactions = reaction_map
            .iter()
            .map(|(key, senders)| MatrixReactionSummary {
                key: key.clone(),
                count: senders.len() as u32,
                senders: senders.keys().cloned().collect(),
                participants: senders
                    .iter()
                    .map(
                        |(user_id, timestamp)| types::chat::MatrixReactionParticipant {
                            user_id: user_id.clone(),
                            timestamp: *timestamp,
                        },
                    )
                    .collect(),
            })
            .collect();
    }
}

pub(super) async fn parse_message_chunk<M: MediaResolver>(
    media_resolver: &M,
    client: &matrix_sdk::Client,
    room_id: &matrix_sdk::ruma::RoomId,
    chunk: Vec<TimelineEvent>,
    reactions_by_target: &mut ReactionAccumulator,
) -> (Vec<MatrixChatMessage>, bool) {
    let mut messages: Vec<MatrixChatMessage> = Vec::new();
    let mut had_utd = false;
    let mut sender_display_names: HashMap<String, Option<String>> = HashMap::new();
    let local_user_id = client.user_id().map(|user_id| user_id.to_string());

    for timeline in chunk {
        let Ok(event) = timeline.raw().deserialize_as::<Value>() else {
            continue;
        };
        let event_type = event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let is_message_event = matches!(event_type, "m.room.message" | "m.room.encrypted");
        let has_redaction_marker = event
            .get("unsigned")
            .and_then(|unsigned| unsigned.get("redacted_because"))
            .is_some();
        let has_empty_message_content = event_type == "m.room.message"
            && event
                .get("content")
                .and_then(Value::as_object)
                .is_some_and(|content| content.is_empty());
        if is_message_event && (has_redaction_marker || has_empty_message_content) {
            messages.push(MatrixChatMessage {
                event_id: event
                    .get("event_id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                in_reply_to_event_id: None,
                sender: event
                    .get("sender")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_owned(),
                timestamp: event.get("origin_server_ts").and_then(Value::as_u64),
                body: String::new(),
                formatted_body: None,
                message_type: Some(String::from("m.text")),
                image_url: None,
                image_width: None,
                image_height: None,
                thumbnail_url: None,
                custom_emojis: Vec::new(),
                reactions: Vec::new(),
                encrypted: false,
                decryption_status: MatrixMessageDecryptionStatus::Plaintext,
                verification_status: MatrixMessageVerificationStatus::Unknown,
                sender_display_name: None,
                edited_at: None,
                is_deleted: true,
                thread_root_id: None,
                thread_reply_count: 0,
                thread_participants: Vec::new(),
                is_falling_back: false,
                mentions_me: false,
                is_sticker: false,
                is_voice: false,
                audio_duration_ms: None,
            });
            continue;
        }

        let encryption_info = timeline.encryption_info();
        let is_utd = timeline.kind.is_utd();
        let decryption_status = if is_utd {
            MatrixMessageDecryptionStatus::UnableToDecrypt
        } else if encryption_info.is_some() {
            MatrixMessageDecryptionStatus::Decrypted
        } else {
            MatrixMessageDecryptionStatus::Plaintext
        };

        if is_utd {
            had_utd = true;
        }

        let verification_status = match encryption_info.map(|info| &info.verification_state) {
            Some(VerificationState::Verified) => MatrixMessageVerificationStatus::Verified,
            Some(VerificationState::Unverified(_)) => MatrixMessageVerificationStatus::Unverified,
            None => MatrixMessageVerificationStatus::Unknown,
        };

        if let Some(parsed_reaction) = parse_reaction_event(&event) {
            reactions_by_target
                .entry(parsed_reaction.target_event_id)
                .or_default()
                .entry(parsed_reaction.key)
                .or_default()
                .entry(parsed_reaction.sender)
                .or_insert(event.get("origin_server_ts").and_then(Value::as_u64));
            continue;
        }

        if let Some(change) = parse_membership_change(&event) {
            let actor_display_name = match sender_display_names.get(&change.actor) {
                Some(cached) => cached.clone(),
                None => {
                    let resolved =
                        resolve_sender_display_name(client, room_id, &change.actor).await;
                    sender_display_names.insert(change.actor.clone(), resolved.clone());
                    resolved
                }
            }
            .unwrap_or_else(|| localpart(&change.actor));
            let body = match change.action {
                "joined" => format!("{} joined the room", change.target_display_name),
                "left" => format!("{} left the room", change.target_display_name),
                "removed" => format!(
                    "{} was removed by {}",
                    change.target_display_name, actor_display_name
                ),
                "banned" => format!(
                    "{} was banned by {}",
                    change.target_display_name, actor_display_name
                ),
                _ => continue,
            };

            messages.push(MatrixChatMessage {
                event_id: event
                    .get("event_id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                in_reply_to_event_id: None,
                sender: change.target_user_id,
                timestamp: event.get("origin_server_ts").and_then(Value::as_u64),
                body,
                formatted_body: None,
                message_type: Some(String::from("m.room.member")),
                image_url: None,
                image_width: None,
                image_height: None,
                thumbnail_url: None,
                custom_emojis: Vec::new(),
                reactions: Vec::new(),
                encrypted: false,
                decryption_status: MatrixMessageDecryptionStatus::Plaintext,
                verification_status: MatrixMessageVerificationStatus::Unknown,
                sender_display_name: Some(change.target_display_name),
                edited_at: None,
                is_deleted: false,
                thread_root_id: None,
                thread_reply_count: 0,
                thread_participants: Vec::new(),
                is_falling_back: false,
                mentions_me: false,
                is_sticker: false,
                is_voice: false,
                audio_duration_ms: None,
            });
            continue;
        }

        if let Some(parsed) = parse_timeline_message(&event, decryption_status, verification_status)
        {
            let mut custom_emojis = Vec::with_capacity(parsed.custom_emojis.len());
            let mut custom_emoji_urls_by_source = HashMap::<String, String>::new();
            for emoji in parsed.custom_emojis {
                let source_url = emoji.url.clone();
                custom_emojis.push(MatrixCustomEmoji {
                    shortcode: emoji.shortcode,
                    // Keep the MXC URL in the event model. Resolving it here
                    // blocks the whole history page on one request per emoji;
                    // the UI resolves it asynchronously through the logged-in
                    // Matrix client after painting the text.
                    url: source_url.clone(),
                    source_url: Some(source_url.clone()),
                });

                if source_url.is_empty() || custom_emoji_urls_by_source.contains_key(&source_url) {
                    continue;
                }

                custom_emoji_urls_by_source.insert(source_url.clone(), source_url);
            }

            let formatted_body = parsed.formatted_body.map(|body| {
                rewrite_formatted_body_custom_emoji(&body, &custom_emoji_urls_by_source)
            });

            let is_video = parsed.message_type.as_deref() == Some("m.video");
            let is_sticker = parsed.is_sticker;
            let is_voice = parsed.voice_duration_ms.is_some()
                || parsed.message_type.as_deref() == Some("m.audio");

            // Videos are downloaded on demand (when the user presses play) to
            // avoid fetching large files for every message in the timeline.
            // We eagerly resolve the thumbnail (poster) so the message shows a
            // preview without downloading the full video. The video's own
            // `image_url` is left unset; only the resolved `asset://` thumbnail
            // is exposed to the frontend.
            let (image_url, thumbnail_url) = if is_video {
                let thumbnail_url = media_resolver
                    .resolve_thumbnail_cache_path(client, &event)
                    .await;
                (None, thumbnail_url)
            } else if matches!(
                parsed.message_type.as_deref(),
                Some("m.image") | Some("m.file")
            ) {
                let image_url = media_resolver
                    .resolve_image_cache_path(client, &event)
                    .await;
                (image_url, None)
            } else {
                (None, None)
            };

            // An edit replaces an earlier message rather than adding one: the
            // new text is applied to the target and the edit itself is not
            // rendered as a separate bubble.
            if parsed.relates_to_rel_type.as_deref() == Some(event_types::rel_types::REPLACE) {
                if let Some(target_event_id) = parsed.relates_to_event_id.as_deref() {
                    if let Some(target) = messages
                        .iter_mut()
                        .find(|message| message.event_id.as_deref() == Some(target_event_id))
                    {
                        target.body = parsed.body.clone();
                        target.formatted_body = formatted_body.clone();
                        target.edited_at = parsed.timestamp;
                    }
                }

                continue;
            }

            let sender_display_name = match sender_display_names.get(&parsed.sender) {
                Some(cached) => cached.clone(),
                None => {
                    let resolved =
                        resolve_sender_display_name(client, room_id, &parsed.sender).await;
                    sender_display_names.insert(parsed.sender.clone(), resolved.clone());
                    resolved
                }
            };

            let mentions_me = local_user_id
                .as_ref()
                .map(|user_id| parsed.mention_user_ids.iter().any(|id| id == user_id))
                .unwrap_or(false);

            let thread_root_id =
                if parsed.relates_to_rel_type.as_deref() == Some(event_types::rel_types::THREAD) {
                    parsed.relates_to_event_id.clone()
                } else {
                    None
                };

            messages.push(MatrixChatMessage {
                event_id: parsed.event_id,
                in_reply_to_event_id: parsed.in_reply_to_event_id,
                sender: parsed.sender,
                timestamp: parsed.timestamp,
                body: parsed.body,
                formatted_body,
                message_type: parsed.message_type,
                image_url,
                image_width: parsed.image_width,
                image_height: parsed.image_height,
                thumbnail_url,
                custom_emojis,
                reactions: Vec::new(),
                encrypted: parsed.encrypted,
                decryption_status: parsed.decryption_status,
                verification_status: parsed.verification_status,
                sender_display_name,
                edited_at: None,
                is_deleted: parsed.is_redacted,
                thread_root_id,
                thread_reply_count: parsed.thread_reply_count,
                thread_participants: parsed.thread_participants,
                is_falling_back: parsed.is_falling_back,
                mentions_me,
                is_sticker,
                is_voice,
                audio_duration_ms: parsed.voice_duration_ms,
            });
        }
    }

    let mut participants_by_thread: HashMap<String, (u32, Vec<String>)> = HashMap::new();
    for message in &messages {
        let Some(root_id) = message.thread_root_id.as_ref() else {
            continue;
        };
        let (count, participants) = participants_by_thread.entry(root_id.clone()).or_default();
        *count = count.saturating_add(1);
        if !participants.iter().any(|sender| sender == &message.sender) {
            participants.push(message.sender.clone());
        }
    }
    for message in &mut messages {
        let Some(event_id) = message.event_id.as_ref() else {
            continue;
        };
        if let Some((count, participants)) = participants_by_thread.get(event_id) {
            message.thread_reply_count = message.thread_reply_count.max(*count);
            message.thread_participants = participants.iter().rev().take(3).cloned().collect();
        }
    }

    apply_reactions(&mut messages, reactions_by_target);

    (messages, had_utd)
}

struct ParsedMembershipChange {
    actor: String,
    target_user_id: String,
    target_display_name: String,
    action: &'static str,
}

fn parse_membership_change(event: &Value) -> Option<ParsedMembershipChange> {
    if event.get("type").and_then(Value::as_str) != Some("m.room.member") {
        return None;
    }

    let actor = event.get("sender").and_then(Value::as_str)?;
    let target = event.get("state_key").and_then(Value::as_str)?;
    let content = event.get("content")?;
    let membership = content.get("membership").and_then(Value::as_str)?;
    let previous_membership = event
        .pointer("/unsigned/prev_content/membership")
        .or_else(|| event.pointer("/prev_content/membership"))
        .and_then(Value::as_str);

    let action = match membership {
        "join" if previous_membership != Some("join") => "joined",
        "leave" if previous_membership != Some("leave") => {
            if actor == target {
                "left"
            } else {
                "removed"
            }
        }
        "ban" if previous_membership != Some("ban") => "banned",
        _ => return None,
    };

    let target_display_name = content
        .get("displayname")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            event
                .pointer("/unsigned/prev_content/displayname")
                .and_then(Value::as_str)
                .filter(|name| !name.trim().is_empty())
        })
        .or_else(|| {
            event
                .pointer("/prev_content/displayname")
                .and_then(Value::as_str)
                .filter(|name| !name.trim().is_empty())
        })
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| localpart(target));

    Some(ParsedMembershipChange {
        actor: actor.to_owned(),
        target_user_id: target.to_owned(),
        target_display_name,
        action,
    })
}

fn localpart(user_id: &str) -> String {
    user_id
        .split(':')
        .next()
        .unwrap_or(user_id)
        .trim_start_matches('@')
        .to_owned()
}

fn rewrite_formatted_body_custom_emoji(
    formatted_body: &str,
    custom_emoji_urls_by_source: &HashMap<String, String>,
) -> String {
    if custom_emoji_urls_by_source.is_empty() {
        return formatted_body.to_owned();
    }

    let mut rewritten = String::with_capacity(formatted_body.len());
    let mut search_index = 0_usize;

    while let Some(relative_start) = formatted_body[search_index..].find("<img") {
        let tag_start = search_index + relative_start;
        let Some(relative_end) = formatted_body[tag_start..].find('>') else {
            rewritten.push_str(&formatted_body[search_index..]);
            return rewritten;
        };

        let tag_end = tag_start + relative_end + 1;
        let tag = &formatted_body[tag_start..tag_end];
        rewritten.push_str(&formatted_body[search_index..tag_start]);
        rewritten.push_str(&rewrite_img_tag(tag, custom_emoji_urls_by_source));
        search_index = tag_end;
    }

    rewritten.push_str(&formatted_body[search_index..]);
    rewritten
}

fn rewrite_img_tag(tag: &str, custom_emoji_urls_by_source: &HashMap<String, String>) -> String {
    // Rewrite src URL for all img elements
    let mut result = tag.to_owned();
    if let Some(source_url) = extract_html_attribute(tag, "src") {
        if let Some(rewritten_src) = custom_emoji_urls_by_source.get(&source_url) {
            result = replace_html_attribute(&result, "src", rewritten_src);
        }
    }

    // Handle width/height dimensions only for emoji
    let is_emoji = result.contains("data-mx-emoticon") || result.contains("data_mx_emoticon");
    if is_emoji {
        let width = extract_html_attribute(&result, "width");
        let height = extract_html_attribute(&result, "height");

        match (width, height) {
            (None, None) => {
                result = add_html_attribute(&result, "width", "32");
                result = add_html_attribute(&result, "height", "32");
            }
            (None, Some(h)) => {
                result = add_html_attribute(&result, "width", &h);
            }
            (Some(w), None) => {
                result = add_html_attribute(&result, "height", &w);
            }
            (Some(_), Some(_)) => {
                // Both present: no change
            }
        }
    }

    result
}

fn replace_html_attribute(tag: &str, attribute: &str, value: &str) -> String {
    let quoted_pattern = format!("{attribute}=\"");
    if let Some(start_index) = tag.find(&quoted_pattern) {
        let value_start = start_index + quoted_pattern.len();
        if let Some(relative_end) = tag[value_start..].find('"') {
            let value_end = value_start + relative_end;
            return format!("{}{}{}", &tag[..value_start], value, &tag[value_end..]);
        }
    }

    let single_quote_pattern = format!("{attribute}='");
    if let Some(start_index) = tag.find(&single_quote_pattern) {
        let value_start = start_index + single_quote_pattern.len();
        if let Some(relative_end) = tag[value_start..].find('\'') {
            let value_end = value_start + relative_end;
            return format!("{}{}{}", &tag[..value_start], value, &tag[value_end..]);
        }
    }

    tag.to_owned()
}

fn add_html_attribute(tag: &str, attribute: &str, value: &str) -> String {
    // First try to replace if it exists
    let quoted_pattern = format!("{attribute}=\"");
    if tag.contains(&quoted_pattern) {
        return replace_html_attribute(tag, attribute, value);
    }

    let single_quote_pattern = format!("{attribute}='");
    if tag.contains(&single_quote_pattern) {
        return replace_html_attribute(tag, attribute, value);
    }

    // Attribute doesn't exist, add it before the closing >
    if let Some(close_pos) = tag.rfind('>') {
        let before_close = &tag[..close_pos];
        let close_char = &tag[close_pos..];
        format!("{} {}=\"{}\"{}", before_close, attribute, value, close_char)
    } else {
        tag.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::rewrite_formatted_body_custom_emoji;

    #[test]
    fn rewrites_custom_emoji_image_src_to_matrix_media() {
        let mut custom_emoji_urls_by_source = HashMap::new();
        custom_emoji_urls_by_source.insert(
            "mxc://media.example.org/wave".to_owned(),
            "matrix-media://localhost/wave".to_owned(),
        );

        let rewritten = rewrite_formatted_body_custom_emoji(
            "<p>hello <img data-mx-emoticon src=\"mxc://media.example.org/wave\" alt=\":wave:\"></p>",
            &custom_emoji_urls_by_source,
        );

        assert_eq!(
            rewritten,
            "<p>hello <img data-mx-emoticon src=\"matrix-media://localhost/wave\" alt=\":wave:\" width=\"32\" height=\"32\"></p>"
        );
    }

    #[test]
    fn adds_dimensions_when_missing() {
        let mut custom_emoji_urls_by_source = HashMap::new();
        custom_emoji_urls_by_source.insert(
            "mxc://media.example.org/smile".to_owned(),
            "matrix-media://localhost/smile".to_owned(),
        );

        let rewritten = rewrite_formatted_body_custom_emoji(
            "<p><img data-mx-emoticon src=\"mxc://media.example.org/smile\" alt=\":smile:\"></p>",
            &custom_emoji_urls_by_source,
        );

        assert!(rewritten.contains("width=\"32\""));
        assert!(rewritten.contains("height=\"32\""));
    }

    #[test]
    fn sets_width_equal_to_height_when_width_missing() {
        let mut custom_emoji_urls_by_source = HashMap::new();
        custom_emoji_urls_by_source.insert(
            "mxc://media.example.org/joy".to_owned(),
            "matrix-media://localhost/joy".to_owned(),
        );

        let rewritten = rewrite_formatted_body_custom_emoji(
            "<p><img data-mx-emoticon src=\"mxc://media.example.org/joy\" alt=\":joy:\" height=\"48\"></p>",
            &custom_emoji_urls_by_source,
        );

        assert!(rewritten.contains("width=\"48\""));
        assert!(rewritten.contains("height=\"48\""));
    }

    #[test]
    fn sets_height_equal_to_width_when_height_missing() {
        let mut custom_emoji_urls_by_source = HashMap::new();
        custom_emoji_urls_by_source.insert(
            "mxc://media.example.org/sad".to_owned(),
            "matrix-media://localhost/sad".to_owned(),
        );

        let rewritten = rewrite_formatted_body_custom_emoji(
            "<p><img data-mx-emoticon src=\"mxc://media.example.org/sad\" alt=\":sad:\" width=\"64\"></p>",
            &custom_emoji_urls_by_source,
        );

        assert!(rewritten.contains("width=\"64\""));
        assert!(rewritten.contains("height=\"64\""));
    }

    #[test]
    fn handles_underscore_data_mx_emoticon_format() {
        let mut custom_emoji_urls_by_source = HashMap::new();
        custom_emoji_urls_by_source.insert(
            "mxc://matrix.luxuride.eu/GPXPlREYzJTkwoPxjpTkfMWg".to_owned(),
            "matrix-media://localhost/GPXPlREYzJTkwoPxjpTkfMWg".to_owned(),
        );

        let rewritten = rewrite_formatted_body_custom_emoji(
            "<p>Test <img data_mx_emoticon=\"\" src=\"mxc://matrix.luxuride.eu/GPXPlREYzJTkwoPxjpTkfMWg\" alt=\":neuro-flushed:\" title=\":neuro-flushed:\" height=\"32\"></img></p>",
            &custom_emoji_urls_by_source,
        );

        assert!(rewritten.contains("matrix-media://localhost/GPXPlREYzJTkwoPxjpTkfMWg"));
        assert!(!rewritten.contains("mxc://matrix.luxuride.eu"));
        assert!(rewritten.contains("width=\"32\""));
        assert!(rewritten.contains("height=\"32\""));
    }

    #[test]
    fn rewrites_non_emoji_img_urls_when_in_map() {
        let mut custom_emoji_urls_by_source = HashMap::new();
        custom_emoji_urls_by_source.insert(
            "mxc://media.example.org/image1".to_owned(),
            "matrix-media://localhost/image1".to_owned(),
        );

        let rewritten = rewrite_formatted_body_custom_emoji(
            "<p>Here is an image: <img src=\"mxc://media.example.org/image1\" alt=\"Some image\"></p>",
            &custom_emoji_urls_by_source,
        );

        // Should rewrite URL since it's in the map
        assert!(rewritten.contains("matrix-media://localhost/image1"));

        // Should NOT add dimensions (not an emoji)
        assert!(!rewritten.contains("width=\"32\""));
        assert!(!rewritten.contains("height=\"32\""));
    }
}
