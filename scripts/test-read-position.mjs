import assert from "node:assert/strict";
import { firstUnreadIndex } from "../src/lib/chats/read-position.ts";
const messages = [
  { eventId: "$read", sender: "@a:s" },
  { eventId: "$own", sender: "@me:s" },
  { eventId: "$first", sender: "@b:s" },
  { eventId: "$last", sender: "@c:s" },
];
assert.equal(firstUnreadIndex(messages, "$read", 99, "@me:s"), 2, "receipt takes precedence over an approximate count");
assert.equal(firstUnreadIndex(messages, null, 2, "@me:s"), 2, "own messages do not consume the incoming unread count");
assert.equal(firstUnreadIndex(messages, null, 500, "@me:s"), 0, "missing marker starts at the oldest available unread message");
assert.equal(firstUnreadIndex(messages, "$last", 1, "@me:s"), -1, "do not put a divider above the read marker");
assert.equal(firstUnreadIndex(messages, "$read", 0, "@me:s"), -1, "no separator for a read room");
console.log("Read-position tests passed (5 cases).");
