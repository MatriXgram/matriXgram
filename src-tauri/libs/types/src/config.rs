/// Deep-link / OAuth callback scheme.
///
/// `matrix` is the scheme the Matrix ecosystem already uses for per-room and
/// per-event permalinks, so links produced by other clients open here too.
/// Note that it collides with Element's registration of the same scheme on
/// Windows; only one app can own it per user session.
pub const CALLBACK_REDIRECT_SCHEME: &str = "matrix";
pub const CALLBACK_REDIRECT_HOST: &str = "oauth-callback";
pub const CALLBACK_REDIRECT_URI: &str = "matrix://oauth-callback";

/// User-agent / device name reported to the homeserver.
pub const DEVICE_DISPLAY_NAME: &str = "Matrix Telegram Desktop";
pub const SYNC_TIMEOUT_SECONDS: u64 = 15;
pub const INITIAL_ROOM_SYNC_TIMEOUT_SECONDS: u64 = 12;
pub const LONG_POLL_SYNC_TIMEOUT_SECONDS: u64 = 25;
pub const WORKER_UNAUTH_SLEEP_SECONDS: u64 = 2;
pub const WORKER_RETRY_INITIAL_DELAY_MS: u64 = 500;
pub const WORKER_STARTUP_RETRY_MAX_DELAY_MS: u64 = 2_000;
pub const WORKER_RETRY_MAX_DELAY_MS: u64 = 15_000;
