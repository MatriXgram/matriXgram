import type { MatrixChatMessage } from "$lib/chats/types";

export function toTime(timestamp: number | null | undefined): string {
  if (!timestamp) {
    return "";
  }

  return new Date(timestamp).toLocaleString();
}

/// Wall-clock time (`14:32`), the format Telegram uses inside a bubble.
export function toClockTime(timestamp: number | null | undefined): string {
  if (!timestamp) {
    return "";
  }

  return new Date(timestamp).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

/// Timestamp for a chat list row: the time today, "Yesterday", the weekday
/// within the last week, and a date beyond that.
export function toChatListTime(timestamp: number | null | undefined): string {
  if (!timestamp) {
    return "";
  }

  const date = new Date(timestamp);
  const now = new Date();

  if (date.toDateString() === now.toDateString()) {
    return toClockTime(timestamp);
  }

  const yesterday = new Date(now.getTime() - 86_400_000);
  if (date.toDateString() === yesterday.toDateString()) {
    return "Yesterday";
  }

  if (now.getTime() - date.getTime() < 6 * 86_400_000) {
    return date.toLocaleDateString([], { weekday: "short" });
  }

  return date.toLocaleDateString([], {
    day: "2-digit",
    month: "2-digit",
    year: "2-digit",
  });
}

/// Label for the date separator between two days of messages.
export function daySeparatorLabel(timestamp: number | null | undefined): string {
  if (!timestamp) {
    return "";
  }

  const date = new Date(timestamp);
  const now = new Date();

  if (date.toDateString() === now.toDateString()) {
    return "Today";
  }

  const yesterday = new Date(now.getTime() - 86_400_000);
  if (date.toDateString() === yesterday.toDateString()) {
    return "Yesterday";
  }

  const sameYear = date.getFullYear() === now.getFullYear();
  return date.toLocaleDateString([], {
    weekday: "long",
    day: "numeric",
    month: "long",
    ...(sameYear ? {} : { year: "numeric" }),
  });
}

export function decryptionLabel(status: MatrixChatMessage["decryptionStatus"]): string {
  if (status === "decrypted") {
    return "Decrypted";
  }

  if (status === "unableToDecrypt") {
    return "Unable to decrypt";
  }

  return "Plaintext";
}

export function verificationLabel(status: MatrixChatMessage["verificationStatus"]): string {
  if (status === "verified") {
    return "Verified sender device";
  }

  if (status === "unverified") {
    return "Unverified sender device";
  }

  return "Verification unknown";
}

export function streamStatusLabel(
  loadingMessages: boolean,
  activeLoadKind: string | null,
  streamMessageCount: number,
): string {
  if (!loadingMessages || !activeLoadKind) {
    return "";
  }

  const noun = streamMessageCount === 1 ? "message" : "messages";
  if (activeLoadKind === "older") {
    return `Loading older ${noun}: ${streamMessageCount}`;
  }

  return `Streaming ${noun}: ${streamMessageCount}`;
}
