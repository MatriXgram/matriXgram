use matrix_sdk::ruma::{OwnedServerName, RoomAliasId, RoomOrAliasId};
use percent_encoding::percent_decode_str;
use std::convert::TryFrom;
use url::Url;

use auth::AuthState;
use std::sync::Arc;
use storage::AppDb;
use types::rooms::{MatrixJoinRoomRequest, MatrixJoinRoomResponse};
use types::Paths;

pub async fn join_room(
    request: MatrixJoinRoomRequest,
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &Arc<AuthState>,
) -> Result<MatrixJoinRoomResponse, String> {
    let client = auth_state.restore_client_and_get(paths, app_db).await?;

    let (room_id_or_alias_raw, via_server_names) = parse_join_link_input(&request.room_id_or_alias);
    let merged_server_names = merge_server_name_candidates(request.server_names, via_server_names);

    let room_id_or_alias = <&RoomOrAliasId>::try_from(room_id_or_alias_raw.as_str())
        .map_err(|e| format!("Invalid room ID or alias: {e}"))?;

    let server_names: Vec<OwnedServerName> = merged_server_names
        .iter()
        .filter_map(|s| {
            <&matrix_sdk::ruma::ServerName>::try_from(s.as_str())
                .ok()
                .map(|name| name.to_owned())
        })
        .collect();

    let response = match client
        .join_room_by_id_or_alias(room_id_or_alias, &server_names)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            // Large federated rooms can commit the join on the homeserver, then
            // time out before the response reaches the client. Reconcile once
            // before reporting failure so the UI does not show a false error.
            let debug_error = format!("{error:?}");
            if debug_error.contains("TimedOut") || debug_error.to_lowercase().contains("timeout") {
                let _ = protocol::sync::sync_once_brief(&client).await;
                let expected_room_id = if room_id_or_alias_raw.starts_with('!') {
                    Some(room_id_or_alias_raw.clone())
                } else {
                    if let Ok(alias) = <&RoomAliasId>::try_from(room_id_or_alias_raw.as_str()) {
                        client
                            .resolve_room_alias(alias)
                            .await
                            .ok()
                            .map(|resolved| resolved.room_id.to_string())
                    } else {
                        None
                    }
                };
                if let Some(expected_room_id) = expected_room_id {
                    if client
                        .joined_rooms()
                        .iter()
                        .any(|room| room.room_id().as_str() == expected_room_id)
                    {
                        let _ = crate::persistence::collect_and_store_chats(app_db, &client).await;
                        return Ok(MatrixJoinRoomResponse {
                            room_id: expected_room_id,
                        });
                    }
                }
                let background_client = client.clone();
                let background_db = Arc::clone(app_db);
                tokio::spawn(async move {
                    for _ in 0..6 {
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        let _ = protocol::sync::sync_once_brief(&background_client).await;
                        let _ = crate::persistence::collect_and_store_chats(
                            &background_db,
                            &background_client,
                        )
                        .await;
                    }
                });
                return Err(String::from(
                    "JOIN_PENDING:Homeserver is still processing this join. The room will appear automatically when membership syncs.",
                ));
            }
            return Err(format!("Failed to join room: {error}"));
        }
    };

    let room_id = response.room_id().to_string();

    // Joining can take a long time already, especially for federated spaces.
    // Do not make the UI wait for the follow-up sync and full room-list rebuild.
    let background_client = client.clone();
    let background_db = Arc::clone(app_db);
    tokio::spawn(async move {
        let _ = protocol::sync::sync_once_brief(&background_client).await;
        let _ =
            crate::persistence::collect_and_store_chats(&background_db, &background_client).await;
    });

    Ok(MatrixJoinRoomResponse { room_id })
}

fn parse_join_link_input(input: &str) -> (String, Vec<String>) {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return (String::new(), Vec::new());
    }

    // Url::parse accepts almost anything, and it reads "room:matrix.org" as the
    // scheme "room" with the path "matrix.org", silently truncating the alias.
    // A bare alias is a valid URL on its own, so the tell is whether there is a
    // path or a fragment: an alias and a room id are colon-separated with no
    // slash in the localpart, while a link form has at least one.
    let parsed = Url::parse(trimmed)
        .ok()
        .filter(|parsed| parsed.path().contains('/') || parsed.fragment().is_some());

    if let Some(parsed) = parsed {
        let mut via_server_names = parse_via_query(parsed.query());

        let mut target = String::new();
        if let Some(fragment) = parsed.fragment() {
            let (fragment_target, fragment_query) = split_target_and_query(fragment);
            via_server_names.extend(parse_via_query(fragment_query));

            target = decode_once(fragment_target.trim_start_matches('/'));
            target = normalize_target_for_scheme(&parsed, target);
        }

        if target.is_empty() {
            let path = parsed.path().trim_start_matches('/');
            target = decode_once(path);
            target = normalize_target_for_scheme(&parsed, target);
        }

        if target.is_empty() {
            target = trimmed.to_owned();
        }

        return (target, dedupe_preserve_order(via_server_names));
    }

    let (raw_target, query) = split_target_and_query(trimmed);
    let target = decode_once(raw_target);
    // Everything reaching here is a bare alias, a room id, or an unsupported
    // scheme. The colon is what makes it an alias.
    let target = if target.contains(':') && !target.starts_with('#') && !target.starts_with('!') {
        format!("#{target}")
    } else {
        target
    };
    let via_server_names = dedupe_preserve_order(parse_via_query(query));
    (target, via_server_names)
}

fn split_target_and_query(value: &str) -> (&str, Option<&str>) {
    match value.split_once('?') {
        Some((target, query)) => (target, Some(query)),
        None => (value, None),
    }
}

fn parse_via_query(query: Option<&str>) -> Vec<String> {
    let mut via_values = Vec::new();
    if let Some(raw_query) = query {
        for (key, value) in url::form_urlencoded::parse(raw_query.as_bytes()) {
            if key == "via" {
                let candidate = value.trim();
                if !candidate.is_empty() {
                    via_values.push(candidate.to_owned());
                }
            }
        }
    }

    via_values
}

fn decode_once(value: &str) -> String {
    percent_decode_str(value)
        .decode_utf8_lossy()
        .trim()
        .to_owned()
}

fn normalize_target_for_scheme(parsed: &Url, target: String) -> String {
    if target.is_empty() {
        return target;
    }

    if target.starts_with('#') || target.starts_with('!') {
        return target;
    }

    if parsed.scheme() == "matrix" {
        if let Some(rest) = target.strip_prefix("r/") {
            return format!("#{rest}");
        }
        if let Some(rest) = target.strip_prefix("room/") {
            return format!("#{rest}");
        }
        if let Some(rest) = target.strip_prefix("roomid/") {
            return rest.to_owned();
        }
        if let Some(rest) = target.strip_prefix("s/") {
            return format!("#{rest}");
        }
        if let Some(rest) = target.strip_prefix("space/") {
            return format!("#{rest}");
        }
    }

    if parsed.scheme() == "matrix" && target.contains(':') {
        return format!("#{target}");
    }

    target
}

fn merge_server_name_candidates(
    primary: Option<Vec<String>>,
    secondary: Vec<String>,
) -> Vec<String> {
    let mut merged = primary.unwrap_or_default();
    merged.extend(secondary);
    dedupe_preserve_order(merged)
}

fn dedupe_preserve_order(values: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for value in values {
        if !deduped.iter().any(|existing| existing == &value) {
            deduped.push(value);
        }
    }
    deduped
}

#[cfg(test)]
mod tests {
    use super::{merge_server_name_candidates, parse_join_link_input};

    #[test]
    fn parses_matrix_to_fragment_via_values() {
        let (target, via) = parse_join_link_input(
            "https://matrix.to/#/%23room:matrix.org?via=matrix.org&via=example.com",
        );

        assert_eq!(target, "#room:matrix.org");
        assert_eq!(via, vec!["matrix.org", "example.com"]);
    }

    #[test]
    fn parses_matrix_join_link_and_adds_alias_prefix() {
        let (target, via) = parse_join_link_input(
            "matrix://join/#example:matrix.org?via=matrix.org&via=matrix.org",
        );

        assert_eq!(target, "#example:matrix.org");
        assert_eq!(via, vec!["matrix.org"]);
    }

    #[test]
    fn parses_raw_alias_with_query_via_values() {
        let (target, via) = parse_join_link_input("#room:matrix.org?via=matrix.org");

        assert_eq!(target, "#room:matrix.org");
        assert_eq!(via, vec!["matrix.org"]);
    }

    #[test]
    fn a_bare_alias_is_not_mistaken_for_a_url_scheme() {
        // "room" parses as a URL scheme, which truncated the alias to
        // "matrix.org".
        let (target, via) = parse_join_link_input("room:matrix.org?via=matrix.org");

        assert_eq!(target, "#room:matrix.org");
        assert_eq!(via, vec!["matrix.org"]);
    }

    #[test]
    fn a_room_id_is_left_alone() {
        let (target, via) = parse_join_link_input("!abc:matrix.org");

        assert_eq!(target, "!abc:matrix.org");
        assert!(via.is_empty());
    }

    #[test]
    fn merges_and_dedupes_server_name_candidates() {
        let merged = merge_server_name_candidates(
            Some(vec!["a.org".to_string(), "b.org".to_string()]),
            vec!["b.org".to_string(), "c.org".to_string()],
        );

        assert_eq!(merged, vec!["a.org", "b.org", "c.org"]);
    }
}
