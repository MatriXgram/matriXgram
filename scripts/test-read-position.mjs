import assert from "node:assert/strict";
import { advanceReadWatermark, countVisibleUnreadMessages, firstUnreadIndex } from "../src/lib/chats/read-position.ts";
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

const watermarkMessages = [
  { eventId: "$90" },
  { eventId: "$95" },
  { eventId: "$100" },
];
assert.equal(advanceReadWatermark(watermarkMessages, "$90", 1), "$95", "the read cursor advances when scrolling down");
assert.equal(advanceReadWatermark(watermarkMessages, "$95", 0), "$95", "scrolling back up never moves the read cursor backwards");
assert.equal(advanceReadWatermark(watermarkMessages, "$95", 2), "$100", "the read cursor can advance again");
assert.equal(advanceReadWatermark(watermarkMessages, "$95", 9), "$95", "invalid viewport indexes do not move the cursor");

const visibleUnreadRows = [
  { eventId: "$marker", sender: "@me:s", messageType: "m.room.message" },
  { eventId: "$visible-1", sender: "@alice:s", messageType: "m.room.message" },
  { eventId: "$joined", sender: "@alice:s", messageType: "m.room.member" },
  { eventId: "$visible-2", sender: "@bob:s", messageType: "m.room.message" },
  { eventId: "$own", sender: "@me:s", messageType: "m.room.message" },
];
assert.equal(countVisibleUnreadMessages(visibleUnreadRows, "$marker", 20, "@me:s"), 2,
  "the badge counts loaded incoming timeline messages, not extra server-side events");
assert.equal(countVisibleUnreadMessages(visibleUnreadRows, "$marker", 20, "@me:s", new Set(["$visible-1"])), 1,
  "locally consumed messages stop contributing immediately");
assert.equal(countVisibleUnreadMessages(visibleUnreadRows, "$missing", 99, "@me:s"), 2,
  "an oversized server count falls back to incoming messages in the loaded window");
assert.equal(countVisibleUnreadMessages(visibleUnreadRows, null, 0, "@me:s", new Set(), new Set(["$visible-2"])), 1,
  "newly received timeline messages are counted without a server unread baseline");

const withMembership = [
  { eventId: "$marker", sender: "@me:s", messageType: "m.room.message" },
  { eventId: "$joined", sender: "@alice:s", messageType: "m.room.member" },
  { eventId: "$message", sender: "@alice:s", messageType: "m.room.message" },
  { eventId: "$left", sender: "@bob:s", messageType: "m.room.member" },
  { eventId: "$newest", sender: "@carol:s", messageType: "m.room.message" },
];
assert.equal(firstUnreadIndex(withMembership, "$marker", 1, "@me:s"), 2, "joined notices do not become the unread boundary");
assert.equal(firstUnreadIndex(withMembership, null, 1, "@me:s"), 4, "left notices do not consume the unread message count");
console.log("Read-position tests passed (15 cases).");
