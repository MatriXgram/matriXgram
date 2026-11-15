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
assert.equal(firstUnreadIndex(messages, null, 500, "@me:s"), -1, "an unread count larger than the loaded window does not guess a boundary");
assert.equal(firstUnreadIndex(messages, "$last", 1, "@me:s"), -1, "do not put a divider above the read marker");
assert.equal(firstUnreadIndex(messages, "$read", 0, "@me:s"), -1, "no separator for a read room");

const withMembership = [
  { eventId: "$marker", sender: "@me:s", messageType: "m.room.message" },
  { eventId: "$joined", sender: "@alice:s", messageType: "m.room.member" },
  { eventId: "$message", sender: "@alice:s", messageType: "m.room.message" },
  { eventId: "$left", sender: "@bob:s", messageType: "m.room.member" },
  { eventId: "$newest", sender: "@carol:s", messageType: "m.room.message" },
];
assert.equal(firstUnreadIndex(withMembership, "$marker", 1, "@me:s"), 2, "joined notices do not become the unread boundary");
assert.equal(firstUnreadIndex(withMembership, null, 1, "@me:s"), 4, "left notices do not consume the unread message count");
console.log("Read-position tests passed (7 cases).");
