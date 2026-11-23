// One game, on the "Make a translation" tab: the translator's steps for it. A translation is a pack, a folder of
// plain text files written in any text editor; this page gives what is around the writing. The user's copy of the
// game, which both saving the script and building need. The game's own script, saved to translate from. The folder
// of their pack, remembered for this game alone, read and checked as far as can be done without the game. And a build
// from that pack, which is the full check of its text, to look at in an emulator.

import { useState } from "react";

import * as backend from "../lib/backend";
import type { Game, GameFile, Listing, OwnFolder, Pack, Recognised } from "../lib/backend";
import { languageName, splitName } from "../lib/format";
import { FaultCard, GameTitle, PackCard, Step, WhatItCanChange, YourCopy } from "./game-parts";

const GUIDE = "https://github.com/bquenin/ventris/blob/main/docs/translating.md";

interface Props {
  game: Game;
  /** Every game, to name the one a file turns out to be. */
  games: Game[];
  /** The folder the user chose for this game's own packs, if they chose one. */
  own: OwnFolder | undefined;
  file: GameFile | undefined;
  gamesFolder: string | null;
  /** Whether the games folder is being looked through. */
  searching: boolean;
  building: boolean;
  onFile: (found: Recognised) => void;
  onForget: (game: string) => void;
  onChooseGamesFolder: () => void;
  onSaveScript: () => void;
  onBuild: (pack: Pack) => void;
  /**
   * Run a command that gives the packs listed after it, such as choosing this game's own folder, after those asked
   * before it, and show its listing. It fails, in words to show, when the command does.
   */
  listWith: (work: () => Promise<Listing>) => Promise<void>;
}

export function TranslatePage(props: Props) {
  const { game, own, file } = props;
  const [reading, setReading] = useState(false);
  const busy = reading || props.searching || props.building;
  const ownPacks = own?.packs ?? [];

  return (
    <div className="mx-auto max-w-3xl px-8 py-7">
      <GameTitle game={game} />
      <p className="mt-4 text-sm text-fg-muted">
        A translation is a pack: a folder of plain text files. You make one with this app and any plain text editor, with
        no programming and no terminal. The guide,{" "}
        <button className="text-accent hover:underline" onClick={() => backend.openHomepage(GUIDE)}>
          Translating a game
        </button>
        , goes through it step by step.
      </p>

      <Step number={1} title="Your copy of the game">
        <YourCopy
          game={game}
          games={props.games}
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

      <Step number={2} title="Save the game's script">
        <div className="card p-4">
          <div className="flex items-start gap-4">
            <p className="min-w-0 flex-1 text-sm text-fg-muted">
              The game's script is its own text, as plain text files, to translate from. It is for your own use: never put
              it in a pack or share it.
            </p>
            <button
              className="button shrink-0"
              disabled={!file || busy}
              title={file ? undefined : "You need your own copy of the game first"}
              onClick={props.onSaveScript}
            >
              Save the game's script…
            </button>
          </div>
          {!file && <p className="mt-3 text-xs text-fg-faint">It can be saved once your copy of the game is found.</p>}
        </div>
      </Step>

      <Step number={3} title="Your pack">
        <YourPack game={game} own={own} file={file} listWith={props.listWith} />
      </Step>

      <Step number={4} title="Build the game to try it">
        <div className="card p-4">
          {ownPacks.length === 0 ? (
            <p className="text-sm text-fg-muted">Once your pack is chosen and can be used, build the game from it here.</p>
          ) : (
            <ul className="space-y-3">
              {ownPacks.map((pack) => (
                <li key={pack.folder} className="flex items-center gap-4">
                  <span className="min-w-0 flex-1 text-sm">
                    <span className="font-semibold">{pack.name}</span>
                    <span className="text-fg-muted">
                      {" "}
                      · {languageName(pack.language)} · version {pack.version}
                    </span>
                  </span>
                  <button
                    className="button-main shrink-0"
                    disabled={!file || busy || !game.can_insert}
                    title={file ? undefined : "You need your own copy of the game first"}
                    onClick={() => props.onBuild(pack)}
                  >
                    Build the game…
                  </button>
                </li>
              ))}
            </ul>
          )}
          <p className="mt-3 text-xs text-fg-faint">
            Building is what checks your text against the game itself: when it refuses something, it says what in words and
            writes nothing. Then open the built game in an emulator and look at your text. If your pack adds characters,
            start from power-on: a saved state holds the game's old font.
          </p>
          {ownPacks.length > 0 && !file && (
            <p className="mt-2 text-xs text-fg-faint">It can be built once your copy of the game is found.</p>
          )}
        </div>
      </Step>

      <WhatItCanChange game={game} />
    </div>
  );
}

/**
 * The folder of the user's pack for this game: choosing it, and the pack as the app reads it. A folder with no pack for
 * this game is not taken, and why is said here.
 */
function YourPack({
  game,
  own,
  file,
  listWith,
}: {
  game: Game;
  own: OwnFolder | undefined;
  file: GameFile | undefined;
  listWith: (work: () => Promise<Listing>) => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const { title } = splitName(game.name);

  async function run(work: () => Promise<Listing>) {
    setBusy(true);
    setRefusal(null);
    try {
      await listWith(work);
    } catch (error) {
      setRefusal(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function choose() {
    const folder = await backend.pickFolder(`Choose the folder your pack for ${title} is in`);
    if (folder) await run(() => backend.chooseOwnPacks(game.id, folder));
  }

  const empty = own && !own.trouble && own.packs.length === 0 && own.faults.length === 0;
  return (
    <>
      <div className="card p-4">
        {own ? (
          <>
            <p className="text-sm text-fg-muted">Your pack for this game is read from this folder:</p>
            <div className="selectable mt-1 break-all font-mono text-xs text-fg-muted">{own.folder}</div>
            {own.trouble && <p className="selectable mt-2 text-sm text-bad">{own.trouble} The ventris packs are still listed.</p>}
            {empty && <p className="mt-2 text-sm text-fg-muted">There is no pack for this game in it now.</p>}
            <div className="mt-3 flex items-center gap-4 text-xs">
              <button className="text-fg-muted hover:text-fg disabled:opacity-40" onClick={() => run(backend.listPacks)} disabled={busy}>
                Look again
              </button>
              <button className="text-fg-muted hover:text-fg disabled:opacity-40" onClick={choose} disabled={busy}>
                Change
              </button>
              <button
                className="text-fg-muted hover:text-fg disabled:opacity-40"
                onClick={() => run(() => backend.forgetOwnPacks(game.id))}
                disabled={busy}
              >
                Stop using it
              </button>
            </div>
          </>
        ) : (
          <div className="flex items-start gap-4">
            <p className="min-w-0 flex-1 text-sm text-fg-muted">
              Choose the folder your pack for this game is in: one you are writing, or one you got elsewhere. It is
              remembered for this game, and listed first under Choose a translation on the Play a translation tab too.
            </p>
            <button className="button shrink-0" onClick={choose} disabled={busy}>
              Choose your pack's folder…
            </button>
          </div>
        )}
        <p className="mt-3 text-xs text-fg-faint">
          Each time the folder is read, its pack is checked as far as can be done without the game: its pack.toml, how its
          files are written, and that each string it changes is one this game has. After you change a file, choose Look
          again.
        </p>
        {refusal && (
          <p role="status" className="selectable mt-3 text-sm text-bad">
            {refusal}
          </p>
        )}
      </div>
      {own && (own.packs.length > 0 || own.faults.length > 0) && (
        <ul className="mt-3 space-y-3">
          {own.packs.map((pack) => (
            <PackCard key={pack.folder} pack={pack} game={game} file={file} />
          ))}
          {own.faults.map((fault) => (
            <FaultCard key={fault.place} fault={fault} side="make" />
          ))}
        </ul>
      )}
    </>
  );
}
