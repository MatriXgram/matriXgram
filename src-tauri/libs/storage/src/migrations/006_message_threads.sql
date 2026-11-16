-- Persist enough relation metadata to render Matrix threads from the local
-- timeline cache. Old cached rows cannot distinguish thread replies from
-- regular replies, so clear the disposable timeline cache during migration.
ALTER TABLE message_cache ADD COLUMN thread_root_id TEXT;
ALTER TABLE message_cache ADD COLUMN thread_reply_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE message_cache ADD COLUMN thread_participants TEXT NOT NULL DEFAULT '[]';
ALTER TABLE message_cache ADD COLUMN is_falling_back INTEGER NOT NULL DEFAULT 0;

DELETE FROM message_cache;
DELETE FROM message_cache_state;
