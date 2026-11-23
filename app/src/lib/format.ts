// How things are worded for someone choosing a game and a pack.

import type { Pack } from "./backend";

/** A language tag as its name in English: "pt-BR" is "Brazilian Portuguese". A tag with no name is shown as it is. */
export function languageName(tag: string): string {
  try {
    const name = new Intl.DisplayNames(["en"], { type: "language" }).of(tag);
    return name && name !== tag ? name : tag;
  } catch {
    return tag;
  }
}

/**
 * A game's name in two parts: "Tales of Destiny 2 (PlayStation 2, Japan)" is the title and,
 * apart, which release it is.
 */
export function splitName(name: string): { title: string; release: string } {
  const parts = /^(.*\S)\s*\(([^()]*)\)$/.exec(name);
  return parts ? { title: parts[1], release: parts[2] } : { title: name, release: "" };
}

/** A count with its noun: "1 pack", "6 packs". */
export function counted(count: number, one: string, many = `${one}s`): string {
  return `${count.toLocaleString("en-US")} ${count === 1 ? one : many}`;
}

/** What a pack changes, in a few words: "21,324 strings: dialogue 18,204, skits 3,120". */
export function whatItChanges(pack: Pack): string {
  const total = pack.changes.reduce((sum, change) => sum + change.strings, 0);
  const parts: string[] = [];
  if (total > 0 || pack.names === 0) {
    const kinds = pack.changes.map((change) => `${change.kind} ${change.strings.toLocaleString("en-US")}`);
    // One kind says nothing more than the total does, beyond its name.
    const detail = pack.changes.length === 1 ? ` in ${pack.changes[0].kind}` : kinds.length ? `: ${kinds.join(", ")}` : "";
    parts.push(`${counted(total, "string")}${detail}`);
  }
  if (pack.names > 0) parts.push(`${counted(pack.names, "character")} renamed`);
  return parts.join(" · ");
}

/** A moment given in seconds since 1970, as a date and time on this computer's clock: "Nov 23, 2026, 5:20 PM". */
export function dateAndTime(seconds: number): string {
  return new Date(seconds * 1000).toLocaleString("en-US", { dateStyle: "medium", timeStyle: "short" });
}

/** A path's last part, for where the whole path is too long to show. */
export function fileName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}
