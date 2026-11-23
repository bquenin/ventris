// The window. It has two sides: playing a translation someone made, and making one. Both are about one game: the
// list of games on the left is the same on both, and the game chosen there stays chosen when the side changes.
//
// Playing starts from the folder the user keeps their games in, which is searched for the games
// there are adapters for. The packs need nothing from the user: the app gets the ventris packs from
// GitHub when it starts, shows the copy it kept from before at once, and the list is brought up to
// date when the fetch is done. After that it is a game, a pack, and a place to write the built
// game. A game's file is only ever read.
//
// Making one is the translator's side, for the same game: they save the game's script from it, write their pack in a
// folder of their own, choose that folder there, and build. The folder belongs to that game alone, and its packs for
// the game are listed first on both sides, marked as from their computer.

import { useCallback, useEffect, useRef, useState } from "react";

import { AboutDialog } from "./components/about-dialog";
import { BuildDialog, type Job } from "./components/build-dialog";
import { GamePage } from "./components/game-page";
import { ScriptDialog, type ScriptJob } from "./components/script-dialog";
import { TranslatePage } from "./components/translate-page";
import { Translations } from "./components/translations";
import * as backend from "./lib/backend";
import type { Game, Listing, OwnFolder, Pack, Remembered } from "./lib/backend";
import { counted, splitName } from "./lib/format";

type Side = "play" | "translate";

const NOTHING: Remembered = { games_folder: null, own_packs: {}, games: {} };

export default function App() {
  const [side, setSide] = useState<Side>("play");
  const [games, setGames] = useState<Game[]>([]);
  const [listing, setListing] = useState<Listing | null>(null);
  const [fetching, setFetching] = useState(false);
  const [fetchTrouble, setFetchTrouble] = useState<string | null>(null);
  const [kept, setKept] = useState<Remembered>(NOTHING);
  const [searching, setSearching] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [trouble, setTrouble] = useState<string | null>(null);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [job, setJob] = useState<Job | null>(null);
  const [script, setScript] = useState<ScriptJob | null>(null);

  /** Run a search of the games folder, showing that it is under way and what came of it. */
  const search = useCallback(async (run: () => Promise<Remembered>) => {
    setSearching(true);
    try {
      setKept(await run());
      setTrouble(null);
    } catch (error) {
      setTrouble(String(error));
    } finally {
      setSearching(false);
    }
  }, []);

  // The commands that change which packs there are and list them after, such as choosing a game's own folder, run one
  // after another, so that each listing is read after what the one before it changed, and the last to arrive is the
  // latest. `asked` counts them, and `running` is how many have not finished.
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const asked = useRef(0);
  const running = useRef(0);

  /** Run a command that gives the packs listed after it, after those asked before it, and show its listing. */
  const listWith = useCallback((work: () => Promise<Listing>): Promise<void> => {
    asked.current += 1;
    running.current += 1;
    const run = queue.current.then(work).finally(() => {
      running.current -= 1;
    });
    queue.current = run.catch(() => {});
    return run.then(setListing);
  }, []);

  /**
   * Fetch the ventris packs from GitHub, without holding up anything else. When that fails, the packs already on
   * this computer stay listed, and why is shown beside them.
   *
   * The fetch is not queued: it can take minutes. Its listing is read at its end, and is only shown when no other
   * command listed the packs while it ran. Otherwise that listing may be read before or after the other's change, so
   * the packs are listed again once the others are done.
   */
  const refreshPacks = useCallback(async () => {
    setFetching(true);
    const [before, idle] = [asked.current, running.current === 0];
    try {
      const listed = await backend.refreshPacks();
      setFetchTrouble(null);
      if (idle && asked.current === before) setListing(listed);
      else await listWith(backend.listPacks).catch(() => {});
    } catch (error) {
      setFetchTrouble(String(error));
    } finally {
      setFetching(false);
    }
  }, [listWith]);

  useEffect(() => {
    let gone = false;
    (async () => {
      try {
        const [listed, before] = await Promise.all([backend.listGames(), backend.remembered()]);
        // The packs kept from before, read without the network; they are fetched again just below.
        const found = await backend.listPacks().catch(() => null);
        if (gone) return;
        setGames(listed);
        setKept(before);
        setListing(found);
        // Start on a game there is something to play in, and that the user has if there is one. A
        // pack that cannot be used counts: its author will want to see why.
        const hasPack = (game: Game) => {
          const own = found?.own.find((folder) => folder.game === game.id);
          return Boolean(
            own?.packs.length ||
              own?.faults.length ||
              found?.packs.some((pack) => pack.game === game.id) ||
              found?.faults.some((fault) => fault.game === game.id),
          );
        };
        const best = listed.find((game) => hasPack(game) && before.games[game.id]) ?? listed.find(hasPack) ?? listed[0];
        setSelected(best?.id ?? null);
        // What is remembered is shown at once; the folder is then looked through for what changed,
        // and the packs are fetched again.
        if (before.games_folder) search(backend.searchGames);
        refreshPacks();
      } catch (error) {
        if (!gone) setTrouble(String(error));
      }
    })();
    return () => {
      gone = true;
    };
  }, [search, refreshPacks]);

  const chooseGamesFolder = useCallback(async () => {
    const folder = await backend.pickFolder("Choose the folder you keep your games in");
    if (folder) await search(() => backend.chooseGamesFolder(folder));
  }, [search]);

  const saveScript = useCallback(
    async (game: Game) => {
      const file = kept.games[game.id];
      if (!file) return;
      const folder = await backend.pickFolder("Choose where to save the game's script");
      if (!folder) return;
      setScript({ game, state: "saving", folder });
      try {
        setScript({ game, state: "done", saved: await backend.saveScript(game.id, file.path, folder) });
      } catch (error) {
        setScript({ game, state: "refused", folder, message: String(error) });
      }
    },
    [kept],
  );

  const build = useCallback(
    async (pack: Pack) => {
      const file = kept.games[pack.game];
      if (!file) return;
      try {
        const output = await backend.pickOutput(await backend.suggestOutput(file.path, pack.folder));
        if (!output) return;
        setJob({ pack, state: "building", output });
        try {
          setJob({ pack, state: "done", built: await backend.build(file.path, pack.folder, output) });
        } catch (error) {
          setJob({ pack, state: "refused", output, message: String(error) });
        }
      } catch (error) {
        setTrouble(String(error));
      }
    },
    [kept],
  );

  const ownOf = (game: Game): OwnFolder | undefined => listing?.own.find((folder) => folder.game === game.id);
  // A game's packs: those in the folder the user chose for it first, then the fetched ones.
  const packsOf = (game: Game) => [
    ...(ownOf(game)?.packs ?? []),
    ...(listing?.packs.filter((pack) => pack.game === game.id) ?? []),
  ];
  // Whether there is anything to count a game's packs in yet: a copy of the ventris packs, or a folder of the user's own.
  const counting = (game: Game) => Boolean(listing && (listing.fetched || ownOf(game)));
  const game = games.find((game) => game.id === selected);
  const have = games.filter((game) => kept.games[game.id]).length;

  return (
    <div className="flex h-full flex-col">
      <header className="flex h-12 shrink-0 items-center gap-6 border-b border-bg-border bg-bg-subtle px-4">
        <div className="flex items-baseline gap-2">
          <span className="font-mono text-base font-semibold tracking-tight text-accent">あA</span>
          <span className="text-sm font-semibold">ventris</span>
        </div>
        <nav className="flex h-full items-stretch gap-1" aria-label="What to do">
          {(
            [
              ["play", "Play a translation"],
              ["translate", "Make a translation"],
            ] as const
          ).map(([name, words]) => (
            <button
              key={name}
              onClick={() => setSide(name)}
              aria-current={side === name ? "page" : undefined}
              className={`border-b-2 px-3 text-sm transition-colors ${
                side === name ? "border-accent text-fg" : "border-transparent text-fg-muted hover:text-fg"
              }`}
            >
              {words}
            </button>
          ))}
        </nav>
        <button className="ml-auto text-sm text-fg-muted hover:text-fg" onClick={() => setAboutOpen(true)}>
          About
        </button>
      </header>

      {trouble && (
        <div role="alert" className="selectable flex items-start gap-3 border-b border-bad/40 bg-bad/10 px-4 py-2 text-sm text-bad">
          <span className="flex-1">{trouble}</span>
          <button className="text-fg-muted hover:text-fg" onClick={() => setTrouble(null)}>
            Dismiss
          </button>
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        <aside className="flex w-72 shrink-0 flex-col border-r border-bg-border bg-bg-subtle">
          <h2 className="px-4 pb-2 pt-4 text-xs font-semibold uppercase tracking-wider text-fg-faint">Games</h2>
          <ul className="min-h-0 flex-1 overflow-y-auto px-2">
            {games.map((listed) => {
              const { title } = splitName(listed.name);
              const packs = packsOf(listed).length;
              const chosen = listed.id === selected;
              const owned = Boolean(kept.games[listed.id]);
              return (
                <li key={listed.id}>
                  <button
                    onClick={() => setSelected(listed.id)}
                    aria-current={chosen ? "true" : undefined}
                    className={`mb-1 w-full rounded-md px-3 py-2 text-left transition-colors ${
                      chosen ? "bg-bg-elevated" : "hover:bg-bg-surface"
                    }`}
                  >
                    <div className="flex items-center gap-2">
                      <span className={`min-w-0 flex-1 text-sm ${chosen ? "font-semibold" : ""}`}>{title}</span>
                      {owned && (
                        <svg viewBox="0 0 16 16" className="h-4 w-4 shrink-0 text-good" role="img" aria-label="You have this game">
                          <title>You have this game</title>
                          <path d="M3 8.5l3.2 3.2L13 5" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                        </svg>
                      )}
                    </div>
                    <div className="mt-0.5 flex items-center gap-2 text-xs text-fg-muted">
                      <span className="truncate">{listed.platform}</span>
                      {counting(listed) && (
                        <span className={packs ? "text-accent" : "text-fg-faint"}>
                          {packs ? counted(packs, "pack") : "no pack yet"}
                        </span>
                      )}
                    </div>
                  </button>
                </li>
              );
            })}
          </ul>

          <div className="space-y-3 border-t border-bg-border px-4 py-3">
            <Folder
              title="Your games"
              path={kept.games_folder}
              note={
                searching
                  ? "Looking through it…"
                  : kept.games_folder
                    ? `${have} of ${counted(games.length, "supported game")} found`
                    : null
              }
              onChange={chooseGamesFolder}
              busy={searching}
              more={
                kept.games_folder && (
                  <button
                    className="text-xs text-fg-muted hover:text-fg disabled:opacity-40"
                    disabled={searching}
                    onClick={() => search(backend.searchGames)}
                  >
                    Look again
                  </button>
                )
              }
            />
            <Translations listing={listing} fetching={fetching} trouble={fetchTrouble} onLookAgain={refreshPacks} />
          </div>
        </aside>

        <main className="min-w-0 flex-1 overflow-y-auto">
          {game && side === "translate" ? (
            <TranslatePage
              key={game.id}
              game={game}
              games={games}
              own={ownOf(game)}
              file={kept.games[game.id]}
              gamesFolder={kept.games_folder}
              searching={searching}
              building={job?.state === "building" || script?.state === "saving"}
              onFile={(found) => setKept((known) => ({ ...known, games: { ...known.games, [found.game]: found.file } }))}
              onForget={(id) => search(() => backend.forgetGame(id))}
              onChooseGamesFolder={chooseGamesFolder}
              onSaveScript={() => saveScript(game)}
              onBuild={build}
              listWith={listWith}
            />
          ) : game ? (
            <GamePage
              key={game.id}
              game={game}
              games={games}
              packs={packsOf(game)}
              own={ownOf(game)}
              listing={listing}
              fetching={fetching}
              fetchTrouble={fetchTrouble}
              onLookAgain={refreshPacks}
              file={kept.games[game.id]}
              gamesFolder={kept.games_folder}
              searching={searching}
              building={job?.state === "building" || script?.state === "saving"}
              onFile={(found) => setKept((known) => ({ ...known, games: { ...known.games, [found.game]: found.file } }))}
              onForget={(id) => search(() => backend.forgetGame(id))}
              onChooseGamesFolder={chooseGamesFolder}
              onBuild={build}
              onMake={() => setSide("translate")}
            />
          ) : (
            !trouble && <p className="p-8 text-sm text-fg-muted">Loading…</p>
          )}
        </main>
      </div>

      {job && <BuildDialog job={job} game={games.find((game) => game.id === job.pack.game)} onClose={() => setJob(null)} />}
      {script && <ScriptDialog job={script} onClose={() => setScript(null)} />}
      {aboutOpen && <AboutDialog onClose={() => setAboutOpen(false)} />}
    </div>
  );
}

/** The folder the user keeps their games in: where it is, a word about it, and a way to change it. */
function Folder(props: {
  title: string;
  path: string | null;
  note: string | null;
  busy: boolean;
  onChange: () => void;
  more?: React.ReactNode;
}) {
  return (
    <div>
      <div className="flex items-center gap-3">
        <span className="flex-1 text-xs font-semibold uppercase tracking-wider text-fg-faint">{props.title}</span>
        {props.more}
        <button className="text-xs text-accent hover:underline disabled:opacity-40" disabled={props.busy} onClick={props.onChange}>
          {props.path ? "Change" : "Choose folder"}
        </button>
      </div>
      <div className={`selectable mt-1 break-all text-xs ${props.path ? "text-fg-muted" : "text-fg-faint"}`}>
        {props.path ?? "No folder chosen yet"}
      </div>
      {props.note && <div className="mt-0.5 text-xs text-fg-faint">{props.note}</div>}
    </div>
  );
}
