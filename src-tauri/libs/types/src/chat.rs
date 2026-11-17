use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatrixMessageLoadKind {
    Initial,
    Older,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatrixMessageDecryptionStatus {
    Plaintext,
    Decrypted,
    UnableToDecrypt,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatrixMessageVerificationStatus {
    Unknown,
    Verified,
    Unverified,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixCustomEmoji {
    pub shortcode: String,
    pub url: String,
    #[serde(default)]
    pub source_url: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixPickerCustomEmoji {
    pub name: String,
    pub shortcodes: Vec<String>,
    pub url: String,
    pub source_url: String,
    pub category: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixReactionSummary {
    pub key: String,
    pub count: u32,
    pub senders: Vec<String>,
    #[serde(default)]
    pub participants: Vec<MatrixReactionParticipant>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixMediaTranscodeProgressEvent {
    pub room_id: String,
    pub file_path: String,
    pub stage: String,
    pub progress: f64,
    pub hardware_mode: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixChatMessage {
    pub event_id: Option<String>,
    pub in_reply_to_event_id: Option<String>,
    pub sender: String,
    pub timestamp: Option<u64>,
    pub body: String,
    pub formatted_body: Option<String>,
    pub message_type: Option<String>,
    pub image_url: Option<String>,
    #[serde(default)]
    pub image_width: Option<u64>,
    #[serde(default)]
    pub image_height: Option<u64>,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub custom_emojis: Vec<MatrixCustomEmoji>,
    #[serde(default)]
    pub reactions: Vec<MatrixReactionSummary>,
    pub encrypted: bool,
    pub decryption_status: MatrixMessageDecryptionStatus,
    pub verification_status: MatrixMessageVerificationStatus,
    /// Display name of the sender, resolved from the room member list.
    #[serde(default)]
    pub sender_display_name: Option<String>,
    /// Set when the message has been edited (`m.replace`).
    #[serde(default)]
    pub edited_at: Option<u64>,
    /// Set when the message has been redacted so the UI can replace its body
    /// with a deleted-message notice.
    #[serde(default)]
    pub is_deleted: bool,
    /// Root event id when the message belongs to a thread (`m.thread`).
    #[serde(default)]
    pub thread_root_id: Option<String>,
    /// Number of replies in the thread this message starts.
    #[serde(default)]
    pub thread_reply_count: u32,
    /// Recent senders in the thread, for compact avatar stacks in the timeline.
    #[serde(default)]
    pub thread_participants: Vec<String>,
    /// A thread reply that is explicitly also shown in the main timeline.
    #[serde(default)]
    pub is_falling_back: bool,
    /// Whether the message mentions the local user (`m.mentions`).
    #[serde(default)]
    pub mentions_me: bool,
    /// Whether the message is a sticker (MSC2545).
    #[serde(default)]
    pub is_sticker: bool,
    /// Whether the message is a voice message (MSC3245).
    #[serde(default)]
    pub is_voice: bool,
    /// Duration of a voice message, in milliseconds.
    #[serde(default)]
    pub audio_duration_ms: Option<u64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixChatMessageStreamEvent {
    pub room_id: String,
    pub stream_id: String,
    pub load_kind: MatrixMessageLoadKind,
    pub sequence: u32,
    pub message: Option<MatrixChatMessage>,
    pub messages: Option<Vec<MatrixChatMessage>>,
    pub next_from: Option<String>,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatMessagesRequest {
    pub room_id: String,
    pub from: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetThreadMessagesRequest {
    pub room_id: String,
    pub thread_root_event_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetRoomMembersRequest {
    pub room_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRoomMember {
    pub user_id: String,
    pub display_name: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixStreamChatMessagesRequest {
    pub room_id: String,
    pub from: Option<String>,
    pub limit: Option<u32>,
    pub stream_id: String,
    pub load_kind: MatrixMessageLoadKind,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSendChatMessageRequest {
    pub room_id: String,
    pub body: String,
    pub in_reply_to_event_id: Option<String>,
    /// When set, send as a reply in this Matrix thread instead of the main timeline.
    #[serde(default)]
    pub thread_root_event_id: Option<String>,
    /// Whether the thread reply should also appear in the room timeline.
    #[serde(default)]
    pub thread_fallback: bool,
    /// Pre-rendered HTML for the message.
    ///
    /// Forwarding a message keeps its formatting by sending the original
    /// `formatted_body` along. The backend sanitises it before it goes out,
    /// because it arrives from the client.
    #[serde(default)]
    pub formatted_body: Option<String>,
    /// Matrix user IDs selected from the room mention picker.
    #[serde(default)]
    pub mentioned_user_ids: Vec<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRedactMessageRequest {
    pub room_id: String,
    pub event_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRedactMessageResponse {
    pub redacted: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetPinnedEventsRequest {
    pub room_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetPinnedEventsResponse {
    pub event_ids: Vec<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetMessagePinnedRequest {
    pub room_id: String,
    pub event_id: String,
    pub pinned: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetMessagePinnedResponse {
    pub pinned: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSendMediaFileRequest {
    pub room_id: String,
    pub file_path: String,
    pub compress_media: bool,
    #[serde(default)]
    pub caption: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixCancelMediaTranscodeRequest {
    pub room_id: String,
    pub file_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixToggleReactionRequest {
    pub room_id: String,
    pub target_event_id: String,
    pub key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetUserAvatarRequest {
    pub room_id: String,
    pub user_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixResolveMediaUrlRequest {
    pub media_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixCopyImageToClipboardRequest {
    pub image_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixResolveVideoUrlRequest {
    pub room_id: String,
    pub event_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixResolveVideoUrlResponse {
    pub video_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixDownloadFileRequest {
    pub room_id: String,
    pub event_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixDownloadFileResponse {
    pub saved: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatMessagesResponse {
    pub room_id: String,
    pub next_from: Option<String>,
    pub messages: Vec<MatrixChatMessage>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixStreamChatMessagesResponse {
    pub stream_id: String,
    pub started: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSendChatMessageResponse {
    pub event_id: String,
    pub formatted_body: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSendMediaFileResponse {
    pub event_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixCancelMediaTranscodeResponse {
    pub cancelled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixToggleReactionResponse {
    pub added: bool,
    pub event_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetUserAvatarResponse {
    pub user_id: String,
    pub image_url: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixResolveMediaUrlResponse {
    pub image_url: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetEmojiPacksResponse {
    pub custom_emoji: Vec<MatrixPickerCustomEmoji>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetUploadLimitResponse {
    pub max_upload_size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixUserProfileResponse {
    pub user_id: String,
    pub display_name: Option<String>,
    pub server_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixOpenDirectChatRequest {
    pub user_id: String,
    #[serde(default = "default_true")]
    pub encrypted: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixOpenDirectChatResponse {
    pub room_id: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixReactionParticipant {
    pub user_id: String,
    pub timestamp: Option<u64>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRoomReadRequest {
    pub room_id: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRoomReadState {
    pub room_id: String,
    pub event_id: Option<String>,
    pub unread_count: u32,
    pub latest_timestamp: Option<u64>,
    pub unread_attention_event_ids: Vec<String>,
    pub read_attention_event_ids: Vec<String>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixAttentionEventRequest {
    pub room_id: String,
    pub event_id: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixReadContextRequest {
    pub room_id: String,
    pub event_id: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixReplyEventRequest {
    pub room_id: String,
    pub event_id: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixReadContextResponse {
    pub room_id: String,
    pub messages: Vec<MatrixChatMessage>,
    pub next_from: Option<String>,
    pub next_to: Option<String>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixMarkReadRequest {
    pub room_id: String,
    pub event_id: String,
    pub timestamp: u64,
    pub remaining: u32,
    pub previous_count: u32,
    pub latest_timestamp: Option<u64>,
}
