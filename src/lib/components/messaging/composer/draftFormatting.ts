// Escape text before constructing the small set of supported Matrix HTML tags.
function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

export function formatDraft(draft: string): string | null {
  let formatted = false;
  function inline(text: string): string {
    const pattern = /(`+)([^`\n]+?)\1|\*\*(.+?)\*\*|~~(.+?)~~|\*(.+?)\*/g;
    let result = "", end = 0;
    for (const match of text.matchAll(pattern)) {
      result += escapeHtml(text.slice(end, match.index));
      const tag = match[2] !== undefined ? "code" : match[3] !== undefined ? "strong" : match[4] !== undefined ? "del" : "em";
      const content = match[2] ?? match[3] ?? match[4] ?? match[5];
      result += `<${tag}>${escapeHtml(content)}</${tag}>`;
      end = match.index! + match[0].length;
      formatted = true;
    }
    return result + escapeHtml(text.slice(end));
  }
  const lines = draft.split("\n");
  const html: string[] = [];
  for (let i = 0; i < lines.length; i++) {
    const fence = lines[i].match(/^(`{3,})\s*$/);
    if (fence) {
      const end = lines.indexOf(fence[1], i + 1);
      if (end !== -1) {
        html.push(`<pre><code>${escapeHtml(lines.slice(i + 1, end).join("\n"))}</code></pre>`);
        formatted = true;
        i = end;
        continue;
      }
    }
    if (lines[i].startsWith("> ")) {
      html.push(`<blockquote>${inline(lines[i].slice(2))}</blockquote>`);
      formatted = true;
    } else html.push(inline(lines[i]));
  }
  return formatted ? html.join("<br>") : null;
}
