// What the program is, and the terms of what it carries. The fonts built into it ask that their
// terms go with every copy, and the app is a copy.

import { useEffect, useState } from "react";

import * as backend from "../lib/backend";
import type { About } from "../lib/backend";

export function AboutDialog({ onClose }: { onClose: () => void }) {
  const [about, setAbout] = useState<About | null>(null);
  useEffect(() => {
    backend.about().then(setAbout, () => setAbout(null));
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center bg-black/60 p-6" onClick={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label="About ventris"
        className="card flex max-h-full w-full max-w-2xl flex-col p-6 shadow-2xl"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="text-lg font-semibold">
          ventris <span className="text-sm font-normal text-fg-faint">{about ? `version ${about.version}` : ""}</span>
        </h2>
        <p className="mt-2 text-sm text-fg-muted">
          This app, ventris, puts a translation of a game's text into your own copy of the game. A translation is a
          pack: a folder of text, with no part of the game in it. No game comes with ventris, and it fetches none. What it
          does fetch, when it starts, is the translation packs, from the ventris packs on GitHub. Play a translation
          builds a game from a pack; Make a translation is for writing one, game by game.
        </p>

        <h3 className="mt-5 text-sm font-semibold">Fonts built in</h3>
        <p className="mt-1 text-sm text-fg-muted">
          Letters a game's own font lacks are drawn from these. Each is passed on under its own terms. The app's icon
          is drawn from the pixels of GNU Unifont.
        </p>
        <div className="mt-3 min-h-0 flex-1 space-y-2 overflow-y-auto pr-1">
          {about?.fonts.map((font) => (
            <details key={font.name} className="rounded-md border border-bg-border bg-bg px-3 py-2 text-sm">
              <summary className="cursor-pointer">
                <span className="font-mono text-fg">{font.name}</span>
                <span className="text-fg-muted"> — {font.about}</span>
                <span className="text-fg-faint"> ({font.license})</span>
              </summary>
              <pre className="selectable mt-2 max-h-64 overflow-auto whitespace-pre-wrap font-mono text-xs text-fg-muted">{font.terms}</pre>
            </details>
          ))}
        </div>

        <div className="mt-5 flex justify-end">
          <button className="button-main" onClick={onClose} autoFocus>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
