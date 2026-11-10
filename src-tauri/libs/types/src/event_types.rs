pub const ROOM_MESSAGE: &str = "m.room.message";
pub const ROOM_ENCRYPTED: &str = "m.room.encrypted";
pub const REACTION: &str = "m.reaction";

pub mod message_types {
    pub const TEXT: &str = "m.text";
    pub const NOTICE: &str = "m.notice";
    pub const EMOTE: &str = "m.emote";
    pub const IMAGE: &str = "m.image";
    pub const VIDEO: &str = "m.video";
    pub const AUDIO: &str = "m.audio";
    pub const FILE: &str = "m.file";
    /// MSC2545 sticker.
    pub const STICKER: &str = "m.sticker";
}

/// Relation types used by `m.relates_to`.
pub mod rel_types {
    /// The event replaces an earlier event (`m.replace`).
    pub const REPLACE: &str = "m.replace";
    /// The event is a thread reply (MSC3440).
    pub const THREAD: &str = "m.thread";
    /// The event is a reaction (`m.annotation`).
    pub const ANNOTATION: &str = "m.annotation";
    /// The event replies to another event.
    pub const REPLY: &str = "m.in_reply_to";
}

/// Key of the MSC3245 voice-message metadata inside an `m.audio` event.
pub const VOICE_METADATA_KEY: &str = "org.matrix.msc3245.voice";
