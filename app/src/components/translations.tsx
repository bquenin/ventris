// Where the translations come from: the ventris packs on GitHub, which the app fetches by itself
// when it starts, and how up to date the copy on this computer is. Shown at the bottom left of the
// window, below the games folder, with a way to fetch them again now.

import type { Listing } from "../lib/backend";
import { dateAndTime } from "../lib/format";

/**
 * Whether a failed fetch got an answer from the server, such as "github.com answered 404 Not Found", and not that the
 * server could not be reached. This computer is then online, and saying to check that would mislead. The words are
 * those `download` in src-tauri/src/fetch.rs gives, and its tests hold them.
 */
export function answeredWithError(trouble: string): boolean {
  return /\banswered \d{3}\b/.test(trouble);
}

interface Props {
  listing: Listing | null;
  /** Whether the packs are being fetched now. */
  fetching: boolean;
  /** Why the last fetch failed, or null. */
  trouble: string | null;
  onLookAgain: () => void;
}

/** In a few words, how the translations on this computer stand, and whether that is a problem. */
export function translationsNote({ listing, fetching, trouble }: Omit<Props, "onLookAgain">): { words: string; bad: boolean } {
  const kept = Boolean(listing?.fetched);
  const since = listing?.checked ? dateAndTime(listing.checked) : null;
  if (fetching) return { words: kept ? "Looking for new translations…" : "Getting the translations…", bad: false };
  if (trouble && kept) {
    const when = since ? `from ${since}` : "the ones kept from before";
    return { words: `The translations could not be refreshed: ${trouble}. The ones shown are ${when}.`, bad: true };
  }
  if (trouble) return { words: `The translations could not be got: ${trouble}.`, bad: true };
  if (kept) return { words: since ? `Up to date as of ${since}` : "Up to date", bad: false };
  return { words: "None yet.", bad: false };
}

export function Translations(props: Props) {
  const note = translationsNote(props);
  return (
    <div>
      <div className="flex items-center gap-3">
        <span className="flex-1 text-xs font-semibold uppercase tracking-wider text-fg-faint">Translations</span>
        <button
          className="text-xs text-fg-muted hover:text-fg disabled:opacity-40"
          disabled={props.fetching}
          onClick={props.onLookAgain}
        >
          Look again
        </button>
      </div>
      <div className="mt-1 text-xs text-fg-muted">From the ventris packs on GitHub</div>
      <div role="status" className={`selectable mt-0.5 text-xs ${note.bad ? "text-bad" : "text-fg-faint"}`}>
        {note.words}
      </div>
    </div>
  );
}
