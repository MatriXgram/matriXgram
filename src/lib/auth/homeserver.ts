const KEY = "matrixgram.login.homeserver";
const LEGACY_KEY = "matrix-telegram.login.homeserver";
function validServer(value: string): boolean {
  try { const url = new URL(value); return ["https:", "http:"].includes(url.protocol) && Boolean(url.hostname) && !url.username && !url.password; }
  catch { return false; }
}
export function readLastHomeserver(): string {
  try {
    const value = localStorage.getItem(KEY) ?? localStorage.getItem(LEGACY_KEY);
    if (value && validServer(value)) {
      localStorage.setItem(KEY, value);
      return value;
    }
  } catch {}
  return "https://matrix.org";
}
export function rememberHomeserver(value: string | null | undefined) {
  if (!value || !validServer(value)) return;
  try { localStorage.setItem(KEY, value); } catch {}
}
