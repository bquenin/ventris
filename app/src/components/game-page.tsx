// One game, on the "Play a translation" tab: whether you have it, and the packs there are for it.
//
// The packs are the ventris packs fetched from GitHub. A folder of the user's own for this game, chosen on the
// "Make a translation" tab, adds its packs for this game: they come first here too, marked as from their computer,
// so that someone who got a pack elsewhere can build it from this tab. A pack that cannot be built from is shown with
// the packs, with the reason, so that a mistake is seen and not an empty list.

import { useState } from "react";

import type { Game, GameFile, Listing, OwnFolder, Pack, Recognised } from "../lib/backend";
import { counted } from "../lib/format";
import { FaultCard, GameTitle, PackCard, Step, WhatItCanChange, YourCopy } from "./game-parts";
import { answeredWithError } from "./translations";

interface Props {
  game: Game;
  /** Every game, to name the one a file turns out to be. */
  games: Game[];
  /** The packs for this game: those in the folder the user chose for it first, then the fetched ones. */
  packs: Pack[];
  /** The folder the user chose for this game's own packs, if they chose one. */
  own: OwnFolder | undefined;
  /** Every pack there is, or null before the packs on this computer have been read. */
  listing: Listing | null;
  /** Whether the ventris packs are being fetched from GitHub now, and why the last fetch failed. */
  fetching: boolean;
  fetchTrouble: string | null;
  file: GameFile | undefined;
  gamesFolder: string | null;
  /** Whether the games folder is being looked through. */
  searching: boolean;
  building: boolean;
  onFile: (found: Recognised) => void;
  onForget: (game: string) => void;
  onChooseGamesFolder: () => void;
  /** Fetch the ventris packs again now. */
  onLookAgain: () => void;
  onBuild: (pack: Pack) => void;
  /** Go to the "Make a translation" tab, on the same game. */
  onMake: () => void;
}

export function GamePage(props: Props) {
  const { game, games, packs, own, listing, fetching, fetchTrouble, file } = props;
  const [reading, setReading] = useState(false);
  const busy = reading || props.searching || props.building;

  const faults = listing?.faults ?? [];
  // A fetched pack that cannot be used is shown on its game's page, and on every page when its game cannot be told.
  // Those in the folder chosen for this game are all this game's.
  const ours = [...(own?.faults ?? []), ...faults.filter((fault) => fault.game === game.id || fault.game === null)];
  // One for a game this version has no adapter for has no page of its own.
  const unplaced = faults.filter((fault) => fault.game !== null && !games.some((listed) => listed.id === fault.game));
  const fetched = Boolean(listing?.fetched);
  const nothing = packs.length === 0 && ours.length === 0;
  const failed = nothing && !fetched && Boolean(fetchTrouble) && !fetching;

  const toMake = (
    <button className="text-accent hover:underline" onClick={props.onMake}>
      Make a translation
    </button>
  );

  return (
    <div className="mx-auto max-w-3xl px-8 py-7">
      <GameTitle game={game} />

      <Step number={1} title="Your copy of the game">
        <YourCopy
          game={game}
          games={games}
          file={file}
          gamesFolder={props.gamesFolder}
          searching={props.searching}
          building={props.building}
          reading={reading}
          onReading={setReading}
          onFile={props.onFile}
          onForget={props.onForget}
          onChooseGamesFolder={props.onChooseGamesFolder}
        />
      </Step>

      <Step number={2} title="Choose a translation">
        {failed ? (
          <div className="card p-4">
            <p className="selectable text-sm text-bad">
              The translations could not be got from the ventris packs on GitHub: {fetchTrouble}.
            </p>
            <p className="mt-1 text-sm text-fg-muted">
              {answeredWithError(fetchTrouble ?? "")
                ? "Choose Look again to try once more."
                : "Check that this computer is online, then choose Look again."}{" "}
              A pack of your own can be tried instead, under Your pack in {toMake}.
            </p>
            <button className="button mt-3" onClick={props.onLookAgain}>
              Look again
            </button>
          </div>
        ) : nothing && !fetched ? (
          <div className="card p-4 text-sm text-fg-muted">Getting the translations from the ventris packs on GitHub…</div>
        ) : nothing ? (
          <div className="card p-4 text-sm text-fg-muted">
            {own
              ? "There is no translation for this game yet, in the ventris packs or in your own folder."
              : "There is no translation for this game in the ventris packs yet."}
          </div>
        ) : (
          <ul className="space-y-3">
            {packs.map((pack) => (
              <PackCard key={pack.folder} pack={pack} game={game} file={file} busy={busy} onBuild={props.onBuild} />
            ))}
            {ours.map((fault) => (
              <FaultCard key={`${fault.from_folder}:${fault.place}`} fault={fault} side="play" />
            ))}
          </ul>
        )}
        {!failed && (
          <p className="mt-3 text-xs text-fg-faint">
            {own ? "Your own pack for this game is chosen in " : "A pack of your own for this game? Choose its folder in "}
            {toMake}.
          </p>
        )}
      </Step>

      <WhatItCanChange game={game} />

      {unplaced.length > 0 && (
        <details className="mt-3 text-sm">
          <summary className="cursor-pointer text-fg-muted hover:text-fg">
            {`${counted(unplaced.length, "translation")} from the ventris packs cannot be used by this version of ventris`}
          </summary>
          <ul className="selectable mt-2 space-y-1 text-xs">
            {unplaced.map((fault) => (
              <li key={fault.place}>
                <span className="font-mono text-fg">{fault.place}</span>
                <span className="text-fg-muted"> — {fault.message}</span>
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}
