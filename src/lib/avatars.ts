const avatarGradients = [
  ["#FF845E", "#D45246"],
  ["#FEBB5B", "#F68136"],
  ["#B694F9", "#6C61DF"],
  ["#9AD164", "#46BA43"],
  ["#53EDD6", "#28C9B7"],
  ["#5CAFFA", "#408ACF"],
  ["#FF8AAC", "#D95574"],
] as const;

export function initialsForName(name: string): string {
  const words = name
    .trim()
    .replace(/^@+/, "")
    .split(/\s+/)
    .map((word) => word.match(/\p{L}/u)?.[0])
    .filter((initial): initial is string => Boolean(initial));
  if (!words.length) return "?";
  const first = words[0] ?? "?";
  if (words.length === 1) return first.toLocaleUpperCase();
  const last = words[words.length - 1] ?? "";
  return `${first}${last}`.toLocaleUpperCase();
}

export function avatarGradient(userId: string): readonly [string, string] {
  let hash = 5381;
  for (let index = 0; index < userId.length; index += 1) {
    hash = (Math.imul(hash, 33) ^ userId.charCodeAt(index)) >>> 0;
  }
  return avatarGradients[hash % avatarGradients.length];
}

export function avatarBackground(userId: string): string {
  const [start, end] = avatarGradient(userId);
  return `linear-gradient(180deg, ${start}, ${end})`;
}

export function senderNameColor(userId: string): string {
  return avatarGradient(userId)[0];
}
