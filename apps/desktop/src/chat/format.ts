/**
 * Time formatting for task rows. The shipped relative-time wording was not
 * captured in detail (docs/screens/new-task.md says "relative time"), so the
 * rebuild uses the conventional compact forms and switches to a short date
 * after a week. Nothing here is persisted; it is presentation only.
 */

export function formatRelative(ms: number, now: number = Date.now()): string {
  const delta = Math.max(0, now - ms);
  const minute = 60_000;
  const hour = 60 * minute;
  const day = 24 * hour;

  if (delta < minute) return "Just now";
  if (delta < hour) return `${Math.floor(delta / minute)}m`;
  if (delta < day) return `${Math.floor(delta / hour)}h`;
  if (delta < 7 * day) return `${Math.floor(delta / day)}d`;

  const date = new Date(ms);
  const sameYear = date.getFullYear() === new Date(now).getFullYear();
  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    ...(sameYear ? {} : { year: "numeric" }),
  });
}

/** The first line of a message, bounded for a task title. */
export function titleFromMessage(text: string, max = 60): string {
  const firstLine = text.split("\n").find((line) => line.trim().length > 0)?.trim() ?? "";
  if (firstLine.length === 0) return "New chat";
  return firstLine.length > max ? `${firstLine.slice(0, max).trimEnd()}…` : firstLine;
}

/**
 * The provider-qualified model label (`REQ-COMPOSER-05`, `docs/12` §9). The
 * stored model id is never rewritten; the provider prefix is presentation only
 * and is omitted when the id already carries it. A session created before the
 * model was known reads "model unknown", never a guessed family.
 */
export function modelLabel(providerId: unknown, modelId: string | null): string {
  const provider = typeof providerId === "string" ? providerId.trim() : "";
  if (!modelId) return provider ? `${provider} · model unknown` : "model unknown";
  if (!provider || modelId.startsWith(`${provider}/`)) return modelId;
  return `${provider}/${modelId}`;
}
