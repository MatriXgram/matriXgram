import type { TimelineMessage } from "./types";

export interface ReplyFallback {
  sender: string | null;
  text: string;
}

/// Read the Matrix rich-reply quote embedded in the reply event itself. This
/// lets the quote render before its referenced event has been fetched.
export function replyFallback(message: TimelineMessage | null | undefined): ReplyFallback {
  if (!message?.inReplyToEventId) return { sender: null, text: "" };

  if (message.formattedBody && typeof DOMParser !== "undefined") {
    const document = new DOMParser().parseFromString(message.formattedBody, "text/html");
    const block = document.querySelector("mx-reply");
    if (block) {
      const userLink = Array.from(block.querySelectorAll("a[href]")).find((link) =>
        /(?:matrix\.to\/#\/|\/user\/|^)@[^/:?#]+:[^/?#]+/i.test(link.getAttribute("href") ?? ""),
      );
      const quote = block.querySelector("blockquote")?.cloneNode(true) as HTMLElement | undefined;
      let text = "";
      if (quote) {
        for (const link of quote.querySelectorAll("a")) link.remove();
        for (const image of quote.querySelectorAll("img")) {
          image.replaceWith(document.createTextNode(image.getAttribute("alt") || "Image"));
        }
        text = (quote.textContent ?? "").replace(/\s+/g, " ").trim();
      }
      if (text || userLink) {
        return { sender: userLink?.textContent?.trim() || null, text: fallbackPreviewText(text) };
      }
    }
  }

  const quoteLines: string[] = [];
  for (const line of message.body.split(/\r?\n/)) {
    if (!line.startsWith(">")) break;
    quoteLines.push(line.replace(/^>\s?/, ""));
  }
  if (!quoteLines.length) return { sender: null, text: "" };
  let sender: string | null = null;
  const senderMatch = quoteLines[0].match(/^<(@[^>]+)>\s*(.*)$/);
  if (senderMatch) {
    sender = senderMatch[1];
    quoteLines[0] = senderMatch[2];
  }
  return { sender, text: fallbackPreviewText(quoteLines.join(" ").trim()) };
}

function fallbackPreviewText(text: string): string {
  return /^[^/\\\r\n]{1,180}\.(?:jpe?g|png|gif|webp|avif|bmp|heic|heif|tiff?)$/i.test(text)
    ? "Image"
    : text;
}

/// Remove the plain-text Matrix quote prefix from a reply before rendering its
/// actual message body.
export function stripPlainReplyFallback(message: TimelineMessage): string {
  if (!message.inReplyToEventId) return message.body;
  const lines = message.body.split(/\r?\n/);
  let quoteEnd = 0;
  while (quoteEnd < lines.length && lines[quoteEnd].startsWith(">")) quoteEnd += 1;
  if (!quoteEnd) return message.body;
  while (quoteEnd < lines.length && !lines[quoteEnd].trim()) quoteEnd += 1;
  return lines.slice(quoteEnd).join("\n").trimStart();
}

/// Strip a Matrix `<mx-reply>` block from formatted HTML, leaving the actual
/// message body. Used when rendering replies and reply previews.
export function stripMxReplyBlock(html: string): string {
  return html.replace(/<mx-reply>[\s\S]*?<\/mx-reply>/i, "").trimStart();
}

/// Derive a short text preview of a message for reply headers. Images show
/// their caption when present, otherwise the fixed "Image" label.
export function replyTextPreview(message: TimelineMessage | null | undefined): string {
  if (!message) {
    return "";
  }

  if (message.messageType === "m.image") {
    const caption = message.body.trim();
    const looksLikeImageFilename = /^[^/\\\r\n]{1,180}\.(?:jpe?g|png|gif|webp|avif|bmp|heic|heif|tiff?)$/i.test(caption);
    if (!caption || looksLikeImageFilename) return "Image";
    return caption.split("\n")[0]?.slice(0, 72) ?? "Image";
  }

  return stripPlainReplyFallback(message).trim().split("\n")[0]?.slice(0, 72) ?? "";
}
