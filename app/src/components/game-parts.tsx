// The parts both of a game's pages are made of: "Play a translation" and "Make a translation" show the same
// game, so they share the block for the user's copy of it, the card of a pack, the card of a pack that cannot
// be used, and what a translation can change in the game.

import { useState } from "react";

import * as backend from "../lib/backend";
import type { Fault, Game, GameFile, Pack, Recognised } from "../lib/backend";
import { languageName, splitName, whatItChanges } from "../lib/format";

type Notice = { tone: "bad" | "plain"; words: string };

/**
 * Whether the user has their copy of the game, and how to give it: the folder they keep their games in is looked
 * through, and choosing a single file is the second way, for a game the search did not find or one kept elsewhere.
 */
export function YourCopy(props: {
  game: Game;
  /** Every game, to name the one a file turns out to be. */
  games: Game[];
  file: GameFile | undefined;
  gamesFolder: string | null;
  /** Whether the games folder is being looked through. */
  searching: boolean;
  /** Whether something else is under way that needs the game's file, such as a build. */
  building: boolean;
  /** Whether a file the user chose is being read, held by the page so that its other buttons wait too. */
  reading: boolean;
  onReading: (reading: boolean) => void;
  onFile: (found: Recognised) => void;
  onForget: (game: string) => void;
  onChooseGamesFolder: () => void;
}) {
  const { game, games, file, gamesFolder, searching, reading } = props;
  const [notice, setNotice] = useState<Notice | null>(null);
  const { title } = splitName(game.name);
  const busy = reading || searching || props.building;
  const releases = game.releases.map((known) => known.name).join("; ");

  async function chooseFile() {
    const path = await backend.pickGameFile(`Choose your copy of ${title}`);
    if (!path) return;
    props.onReading(true);
    setNotice(null);
    try {
      const found = await backend.recogniseGame(path);
      props.onFile(found);
      if (found.game !== game.id) {
        const other = games.find((listed) => listed.id === found.game);
        setNotice({
          tone: "plain",
          words: `That file is ${other ? splitName(other.name).title : found.game}, not ${title}. It is now set as that game's file.`,
        });
      }
    } catch (error) {
      setNotice({ tone: "bad", words: String(error) });
    } finally {
      props.onReading(false);
    }
  }

  return (
    <div className="card p-4">
      {file ? (
        <>
          <div className="flex items-center gap-2 text-sm">
            <Tick />
            <span>
              {file.chosen ? "Chosen by you" : "Found in your games folder"}
              {file.release ? `: ${file.release}` : ""}
            </span>
          </div>
          <div className="selectable mt-1 break-all font-mono text-xs text-fg-muted">{file.path}</div>
          <div className="mt-3 flex items-center gap-4 text-xs">
            <button className="text-fg-muted hover:text-fg disabled:opacity-40" onClick={chooseFile} disabled={busy}>
              {reading ? "Reading the file…" : "Use another file…"}
            </button>
            {file.chosen && (
              <button className="text-fg-muted hover:text-fg disabled:opacity-40" onClick={() => props.onForget(game.id)} disabled={busy}>
                {gamesFolder ? "Go back to my games folder" : "Forget it"}
              </button>
            )}
          </div>
        </>
      ) : !gamesFolder ? (
        <>
          <p className="text-sm text-fg-muted">
            No game comes with ventris. Tell it where you keep yours: it looks through that folder, and the folders
            inside it, for the games it can translate. Files are read to see which game they are, and never changed.
          </p>
          <div className="mt-3 flex flex-wrap items-center gap-4">
            <button className="button-main" onClick={props.onChooseGamesFolder} disabled={busy}>
              {searching ? "Looking through it…" : "Choose your games folder…"}
            </button>
            <button className="text-xs text-fg-muted hover:text-fg disabled:opacity-40" onClick={chooseFile} disabled={busy}>
              {reading ? "Reading the file…" : "or choose this game's file"}
            </button>
          </div>
        </>
      ) : searching ? (
        <p className="text-sm text-fg-muted">Looking through your games folder…</p>
      ) : (
        <>
          <p className="text-sm">This game was not found in your games folder.</p>
          <p className="mt-1 text-sm text-fg-muted">
            This is the release ventris knows: {releases}. A copy that was changed, such as one with a translation
            already applied, is not the same file and is not recognised. If you keep the game somewhere else, choose its
            file.
          </p>
          <button className="button mt-3" onClick={chooseFile} disabled={busy}>
            {reading ? "Reading the file…" : "Choose the game's file…"}
          </button>
        </>
      )}
      {reading && <p className="mt-2 text-xs text-fg-faint">A disc is read whole, which takes a little while.</p>}
      {notice && (
        <p role="status" className={`selectable mt-3 text-sm ${notice.tone === "bad" ? "text-bad" : "text-fg-muted"}`}>
          {notice.words}
        </p>
      )}
    </div>
  );
}

/**
 * A pack that can be built from: what it says about itself and what it changes, with a button to build the game from
 * it when `onBuild` is given.
 */
export function PackCard(props: {
  pack: Pack;
  game: Game;
  file: GameFile | undefined;
  busy?: boolean;
  onBuild?: (pack: Pack) => void;
}) {
  const { pack, file } = props;
  return (
    <li className="card p-4">
      <div className="flex items-start gap-4">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="rounded bg-accent/15 px-1.5 py-0.5 text-xs font-semibold text-accent">{languageName(pack.language)}</span>
            <span className="text-base font-semibold">{pack.name}</span>
            <span className="text-xs text-fg-faint">version {pack.version}</span>
            {pack.from_folder && <FromYourComputer />}
          </div>
          {pack.description && <p className="selectable mt-2 text-sm text-fg-muted">{pack.description}</p>}
          <dl className="mt-3 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
            <dt className="text-fg-faint">By</dt>
            <dd className="text-fg-muted">{pack.authors.join(", ")}</dd>
            <dt className="text-fg-faint">Changes</dt>
            <dd className="text-fg-muted">{whatItChanges(pack)}</dd>
            <dt className="text-fg-faint">Licence</dt>
            <dd className="text-fg-muted">{pack.license ?? "none stated"}</dd>
            {pack.homepage && /^https?:\/\//i.test(pack.homepage) && (
              <>
                <dt className="text-fg-faint">Home</dt>
                <dd>
                  <button className="break-all text-left text-accent hover:underline" onClick={() => backend.openHomepage(pack.homepage!)}>
                    {pack.homepage}
                  </button>
                </dd>
              </>
            )}
          </dl>
        </div>
        {props.onBuild && (
          <button
            className="button-main shrink-0"
            disabled={!file || props.busy || !props.game.can_insert}
            title={file ? undefined : "You need your own copy of the game first"}
            onClick={() => props.onBuild?.(pack)}
          >
            Build the game…
          </button>
        )}
      </div>
      {props.onBuild && !file && <p className="mt-3 text-xs text-fg-faint">It can be built once your copy of the game is found.</p>}
    </li>
  );
}

/**
 * A pack that cannot be built from, with the reason. What to do about it depends on where it is: one of the ventris
 * packs cannot be corrected by the user; one of their own is corrected and read again on the Make a translation tab.
 */
export function FaultCard({ fault, side }: { fault: Fault; side: "play" | "make" }) {
  const lookAgain = side === "make" ? "choose Look again" : "choose Look again under Your pack, in Make a translation";
  let advice: string;
  if (!fault.from_folder) {
    advice = "It is one of the ventris packs on GitHub, and this version of ventris cannot use it. A newer version may.";
  } else if (fault.game) {
    advice = `This pack has a mistake in it. Correct it, then ${lookAgain}.`;
  } else {
    advice = `Its pack.toml cannot be read far enough to say which game it is for. Correct it, then ${lookAgain}.`;
  }
  return (
    <li className="card border-bad/40 p-4">
      <div className="flex flex-wrap items-center gap-2">
        <span className="rounded bg-bad/15 px-1.5 py-0.5 text-xs font-semibold text-bad">Cannot be used</span>
        <span className="selectable font-mono text-sm text-fg">{fault.place}</span>
        {fault.from_folder && <FromYourComputer />}
      </div>
      <p className="selectable mt-2 text-sm text-bad">{fault.message}</p>
      <p className="mt-2 text-xs text-fg-faint">{advice}</p>
    </li>
  );
}

/** The small label on a pack that comes from the folder the user chose for its game. */
export function FromYourComputer() {
  return <span className="rounded bg-bg-elevated px-1.5 py-0.5 text-xs text-fg-muted">From your computer</span>;
}

/**
 * What a pack can change in each body of the game's text, and the limits of each, with a last row for the characters'
 * names in a game whose pack can rename them.
 */
export function WhatItCanChange({ game }: { game: Game }) {
  return (
    <details className="mt-8 text-sm">
      <summary className="cursor-pointer text-fg-muted hover:text-fg">What a translation can change in this game</summary>
      <table className="mt-3 w-full border-collapse text-left text-xs">
        <thead>
          <tr className="border-b border-bg-border text-fg-faint">
            <th className="py-1.5 pr-4 font-normal">Text</th>
            <th className="py-1.5 pr-4 font-normal">A pack can</th>
            <th className="py-1.5 font-normal">Limits</th>
          </tr>
        </thead>
        <tbody>
          {game.kinds.map((kind) => (
            <tr key={kind.name} className="border-b border-bg-border/60 align-top">
              <td className="py-1.5 pr-4 text-fg">{kind.name}</td>
              <td className="py-1.5 pr-4 text-fg-muted">
                {!kind.insert
                  ? "not change it yet"
                  : [
                      kind.grow
                        ? `make strings longer${kind.max_length ? `, up to ${kind.max_length} characters` : ""}`
                        : "change strings, no longer than the game's own",
                      kind.new_characters ? "use new characters" : "",
                    ]
                      .filter(Boolean)
                      .join("; ")}
              </td>
              <td className="selectable py-1.5 text-fg-muted">{kind.limits}</td>
            </tr>
          ))}
          {game.sets_names && (
            <tr className="border-b border-bg-border/60 align-top">
              <td className="py-1.5 pr-4 text-fg">characters' names</td>
              <td className="py-1.5 pr-4 text-fg-muted">rename them, in its names.txt file</td>
              <td className="selectable py-1.5 text-fg-muted">{game.name_limits}</td>
            </tr>
          )}
        </tbody>
      </table>
    </details>
  );
}

/** The heading of the game's title and release, at the top of both pages. */
export function GameTitle({ game }: { game: Game }) {
  const { title, release } = splitName(game.name);
  return (
    <>
      <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
      <p className="mt-1 text-sm text-fg-muted">
        {[release, `written in ${languageName(game.source_language)}`].filter(Boolean).join(" · ")}
      </p>
    </>
  );
}

/** One numbered step of a page. */
export function Step({ number, title, children }: { number: number; title: string; children: React.ReactNode }) {
  return (
    <section className="mt-7">
      <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
        <span className="flex h-5 w-5 items-center justify-center rounded-full bg-bg-elevated text-xs text-fg-muted">{number}</span>
        {title}
      </h2>
      {children}
    </section>
  );
}

export function Tick() {
  return (
    <svg viewBox="0 0 16 16" className="h-4 w-4 text-good" aria-hidden="true">
      <path d="M3 8.5l3.2 3.2L13 5" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
