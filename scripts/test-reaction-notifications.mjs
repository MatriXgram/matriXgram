import assert from "node:assert/strict";
import { get } from "svelte/store";
import {
  clearReactionNotices,
  consumeReactionTarget,
  initializeReactionNotices,
  reactionNotices,
  recordReactionNotice,
  removeReactionNotice,
} from "../src/lib/chats/reaction-notifications.ts";

const storage = new Map();
globalThis.localStorage = {
  getItem: (key) => storage.get(key) ?? null,
  setItem: (key, value) => storage.set(key, value),
};
let now = 1000;
Date.now = () => now;
const base = {
  userId: "@me:example.org", roomId: "!room:example.org", eventId: "$reaction1",
  targetEventId: "$message1", sender: "@alice:example.org", senderName: "Alice", key: "👍", timestamp: 1001,
};
initializeReactionNotices(base.userId);
recordReactionNotice({ ...base, eventId: "$old", timestamp: 999 });
recordReactionNotice({ ...base, eventId: "$self", sender: base.userId });
assert.equal(get(reactionNotices).length, 0, "ignore history and own reactions");
recordReactionNotice(base);
recordReactionNotice(base);
recordReactionNotice({ ...base, eventId: "$reaction2", sender: "@bob:example.org" });
assert.equal(get(reactionNotices).length, 2, "deduplicate by event ID, not target");
clearReactionNotices();
now = 2000;
initializeReactionNotices(base.userId);
assert.equal(get(reactionNotices).length, 2, "restore unread reactions on restart");
recordReactionNotice({ ...base, eventId: "$offline", targetEventId: "$message2", timestamp: 1500 });
assert.equal(get(reactionNotices).length, 3, "accept reactions since previous start");
consumeReactionTarget(base.roomId, base.targetEventId);
assert.deepEqual(get(reactionNotices).map((notice) => notice.eventId), ["$offline"], "consume reactions only on the visited message");
recordReactionNotice(base);
assert.equal(get(reactionNotices).length, 1, "consumed reactions do not reappear on sync replay");
removeReactionNotice(base.userId, "$offline");
removeReactionNotice(base.userId, "$redacted-early");
recordReactionNotice({ ...base, eventId: "$redacted-early", timestamp: 1501 });
assert.equal(get(reactionNotices).length, 0, "redactions remove or suppress notices");
initializeReactionNotices("@other:example.org");
recordReactionNotice(base);
assert.equal(get(reactionNotices).length, 0, "notifications are isolated by account");
console.log("Reaction notification state tests passed");
