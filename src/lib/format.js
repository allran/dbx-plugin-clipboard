export function formatTime(ms) {
  if (!ms) return "";
  try {
    return new Intl.DateTimeFormat(undefined, {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    }).format(new Date(ms));
  } catch {
    return new Date(ms).toLocaleString();
  }
}

export function previewText(value) {
  const normalized = String(value ?? "").replace(/\r\n/g, "\n");
  if (normalized.length <= 240) return normalized;
  return `${normalized.slice(0, 240)}…`;
}

export function itemMatches(item, needle) {
  if (!needle) return true;
  // `text` already joins file paths for files items; still scan `paths` for safety.
  const parts = [item.text || "", item.kind || ""];
  if (Array.isArray(item.paths)) parts.push(...item.paths);
  return parts.join("\n").toLowerCase().includes(needle);
}

export function fileName(path) {
  return String(path).split(/[/\\]/).pop() || path;
}
