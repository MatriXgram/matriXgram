import assert from "node:assert/strict";
import { initialsForName } from "../src/lib/avatars.ts";

assert.equal(initialsForName("😀 Alice Smith"), "AS", "emoji before a name is ignored");
assert.equal(initialsForName("⭐️Alice"), "A", "symbols attached to a name are ignored");
assert.equal(initialsForName("🇺🇸 Bob Jones"), "BJ", "flag emoji are ignored");
assert.equal(initialsForName("Élodie Durand"), "ÉD", "Unicode letters are preserved");
assert.equal(initialsForName("🎉✨"), "?", "emoji-only names use the fallback");

console.log("Avatar initials tests passed (5 cases).");
