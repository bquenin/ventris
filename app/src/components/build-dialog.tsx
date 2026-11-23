// A build, from the moment it starts to what came of it. It stays over the window while the
// game is being written, since nothing else should be started meanwhile.

import { useEffect } from "react";

import * as backend from "../lib/backend";
import type { Built, Game, Pack } from "../lib/backend";
import { splitName } from "../lib/format";

export type Job =
  | { pack: Pack; state: "building"; output: string }
  | { pack: Pack; state: "done"; built: Built }
  | { pack: Pack; state: "refused"; output: string; message: string };

export function BuildDialog({ job, game, onClose }: { job: Job; game: Game | undefined; onClose: () => void }) {
  const busy = job.state === "building";
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [busy, onClose]);

  const title = game ? splitName(game.name).title : job.pack.game;
  const what = `${title} with ${job.pack.name} ${job.pack.version}`;

  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center bg-black/60 p-6">
      <div role="dialog" aria-modal="true" aria-label="Building the game" className="card w-full max-w-xl p-6 shadow-2xl">
        {job.state === "building" && (
          <>
            <h2 className="text-lg font-semibold">Building…</h2>
            <p className="mt-1 text-sm text-fg-muted">{what}</p>
            <div className="mt-5 h-1.5 overflow-hidden rounded-full bg-bg-elevated" role="progressbar" aria-label="Building">
              <div className="h-full w-1/3 animate-[slide_1.4s_ease-in-out_infinite] rounded-full bg-accent" />
            </div>
            <p className="mt-4 text-xs text-fg-faint">
              The whole game is written again with the pack's text in it. A cartridge takes a moment, a disc a little longer.
            </p>
            <style>{`@keyframes slide { 0% { transform: translateX(-100%) } 100% { transform: translateX(300%) } }`}</style>
          </>
        )}

        {job.state === "done" && (
          <>
            <h2 className="text-lg font-semibold">{job.built.identical ? "Built, but nothing changed" : "The translated game is ready"}</h2>
            <p className="mt-1 text-sm text-fg-muted">{what}</p>
            <div className="selectable mt-4 break-all rounded-md bg-bg px-3 py-2 font-mono text-xs text-fg">{job.built.output}</div>
            {job.built.identical ? (
              <p className="mt-3 text-sm text-fg-muted">The pack changes no string of this game, so the file is the same as your own.</p>
            ) : (
              <p className="mt-3 text-sm text-fg-muted">
                Open it in your emulator or on your console as you would the original. Your own copy is untouched.
              </p>
            )}
            <details className="mt-4 text-sm">
              <summary className="cursor-pointer text-fg-muted hover:text-fg">What was changed</summary>
              <pre className="selectable mt-2 max-h-48 overflow-auto whitespace-pre-wrap rounded-md bg-bg px-3 py-2 font-mono text-xs text-fg-muted">
                {[...job.built.lines, `sha1:      ${job.built.sha1}`].join("\n")}
              </pre>
            </details>
            <div className="mt-6 flex justify-end gap-2">
              <button className="button" onClick={() => backend.showInFolder(job.built.output)}>
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
            <h2 className="text-lg font-semibold">The game could not be built</h2>
            <p className="mt-1 text-sm text-fg-muted">{what}</p>
            <p role="alert" className="selectable mt-4 rounded-md border border-bad/40 bg-bad/10 px-3 py-2 text-sm text-bad">
              {job.message}
            </p>
            <p className="mt-3 text-sm text-fg-muted">
              Your own copy is untouched. If the pack is the cause, its authors are the ones to tell.
            </p>
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
