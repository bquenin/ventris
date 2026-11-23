// Saving a game's script, from the moment it starts to what came of it. Like a build, it stays
// over the window while the game is read, since nothing else should be started meanwhile.

import { useEffect } from "react";

import * as backend from "../lib/backend";
import type { Game, Saved } from "../lib/backend";
import { counted, splitName } from "../lib/format";

export type ScriptJob =
  | { game: Game; state: "saving"; folder: string }
  | { game: Game; state: "done"; saved: Saved }
  | { game: Game; state: "refused"; folder: string; message: string };

export function ScriptDialog({ job, onClose }: { job: ScriptJob; onClose: () => void }) {
  const busy = job.state === "saving";
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [busy, onClose]);

  const { title } = splitName(job.game.name);

  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center bg-black/60 p-6">
      <div role="dialog" aria-modal="true" aria-label="Saving the game's script" className="card w-full max-w-xl p-6 shadow-2xl">
        {job.state === "saving" && (
          <>
            <h2 className="text-lg font-semibold">Saving the script…</h2>
            <p className="mt-1 text-sm text-fg-muted">{title}</p>
            <div className="mt-5 h-1.5 overflow-hidden rounded-full bg-bg-elevated" role="progressbar" aria-label="Saving">
              <div className="h-full w-1/3 animate-[slide_1.4s_ease-in-out_infinite] rounded-full bg-accent" />
            </div>
            <p className="mt-4 text-xs text-fg-faint">
              The game's text is read and written out as text files, then checked against the game. A cartridge takes a
              moment, a disc a little longer.
            </p>
            <style>{`@keyframes slide { 0% { transform: translateX(-100%) } 100% { transform: translateX(300%) } }`}</style>
          </>
        )}

        {job.state === "done" && (
          <>
            <h2 className="text-lg font-semibold">The game's script is saved</h2>
            <p className="mt-1 text-sm text-fg-muted">
              {title}, in {counted(job.saved.files, "text file")}
            </p>
            <div className="selectable mt-4 break-all rounded-md bg-bg px-3 py-2 font-mono text-xs text-fg">{job.saved.folder}</div>
            <p className="mt-3 text-sm text-fg-muted">
              Open the files in any plain text editor. They are the game's own text: keep them for your own use, and never
              put them in a pack or share them.
            </p>
            <details className="mt-4 text-sm">
              <summary className="cursor-pointer text-fg-muted hover:text-fg">What was written</summary>
              <pre className="selectable mt-2 max-h-48 overflow-auto whitespace-pre-wrap rounded-md bg-bg px-3 py-2 font-mono text-xs text-fg-muted">
                {job.saved.lines.join("\n")}
              </pre>
            </details>
            <div className="mt-6 flex justify-end gap-2">
              <button className="button" onClick={() => backend.showInFolder(job.saved.folder)}>
                Show in folder
              </button>
              <button className="button-main" onClick={onClose} autoFocus>
                Done
              </button>
            </div>
          </>
        )}

        {job.state === "refused" && (
          <>
            <h2 className="text-lg font-semibold">The script could not be saved</h2>
            <p className="mt-1 text-sm text-fg-muted">{title}</p>
            <p role="alert" className="selectable mt-4 rounded-md border border-bad/40 bg-bad/10 px-3 py-2 text-sm text-bad">
              {job.message}
            </p>
            <p className="mt-3 text-sm text-fg-muted">Your own copy of the game is untouched.</p>
            <div className="mt-6 flex justify-end">
              <button className="button-main" onClick={onClose} autoFocus>
                Close
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
