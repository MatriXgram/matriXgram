/** Extract only user permalinks; room/event links retain normal navigation. */
export function userIdFromMatrixLink(href: string): string | null {
  try {
    const url = new URL(href);
    let id: string;
    if ((url.protocol === "https:" || url.protocol === "http:") && url.hostname === "matrix.to" && !url.username && !url.password) {
      if (!url.hash.startsWith("#/")) return null;
      id = decodeURIComponent(url.hash.slice(2).split("?")[0].split("/")[0]);
    } else if (url.protocol === "matrix:") {
      const match = url.pathname.match(/^(?:u|user)\/([^/]+)$/);
      if (!match) return null;
      id = "@" + decodeURIComponent(match[1]);
    } else return null;
    return /^@[^:\s/?#]+:[^\s/?#]+$/.test(id) ? id : null;
  } catch { return null; }
}
