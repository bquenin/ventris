import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as backend from "../lib/backend";
import type { Game, GameFile, Listing, OwnFolder, Pack } from "../lib/backend";
import { GamePage } from "./game-page";
import { TranslatePage } from "./translate-page";
import { WhatItCanChange } from "./game-parts";
import { ScriptDialog } from "./script-dialog";
import { answeredWithError, Translations, translationsNote } from "./translations";

// The commands behind the page, so that a test can say what they answer and see what was asked.
vi.mock("../lib/backend", async (original) => ({
  ...(await original<typeof import("../lib/backend")>()),
  pickFolder: vi.fn(),
  chooseOwnPacks: vi.fn(),
  forgetOwnPacks: vi.fn(),
  listPacks: vi.fn(),
}));

const game: Game = {
  id: "top-sfc",
  name: "Tales of Phantasia (Super Famicom, Japan)",
  platform: "Super Famicom / SNES",
  source_language: "ja",
  releases: [{ sha1: "0000000000000000000000000000000000000001", name: "Japan" }],
  kinds: [
    {
      name: "menu",
      insert: true,
      grow: true,
      new_characters: true,
      max_length: 26,
      limits: "a new character takes the tile of a kana that no menu string uses any more",
    },
  ],
  can_insert: true,
  sets_names: true,
  name_limits: "a name is 1 to 15 characters, without tags",
};

const other: Game = { ...game, id: "tod2-ps2", name: "Tales of Destiny 2 (PlayStation 2, Japan)" };

const file: GameFile = { path: "C:\\games\\top.zip", release: "Japan", sha1: game.releases[0].sha1, chosen: false };

const fetchedPack: Pack = {
  folder: "C:\\cache\\packs\\current\\top-sfc\\en",
  place: "top-sfc/en",
  game: "top-sfc",
  language: "en",
  name: "Tales of Phantasia in English",
  version: "0.9",
  authors: ["Machine translated, directed by bquenin"],
  license: "CC-BY-NC-SA-4.0",
  description: "Machine translated and not yet reviewed by a human translator.",
  homepage: null,
  changes: [{ kind: "menu", strings: 6 }],
  names: 0,
  from_folder: false,
};

const ownPack: Pack = { ...fetchedPack, folder: "C:\\mine\\fr", place: "fr", language: "fr", name: "Mine", from_folder: true };

const empty: Listing = { own: [], fetched: false, checked: 0, packs: [], faults: [] };

const ownFolder: OwnFolder = { game: "top-sfc", folder: "C:\\mine", trouble: null, packs: [], faults: [] };

const withFaults: Listing = {
  ...empty,
  own: [
    {
      ...ownFolder,
      faults: [
        { place: "fr", game: "top-sfc", message: "999 is not a part of this game's script", from_folder: true },
        { place: "de", game: null, message: "pack.toml: line 1: missing field `language`", from_folder: true },
      ],
    },
    {
      ...ownFolder,
      game: "tod2-ps2",
      faults: [{ place: "en", game: "tod2-ps2", message: "string 06816.9999 does not exist", from_folder: true }],
    },
  ],
  fetched: true,
  checked: 1795467600,
  faults: [
    { place: "broken/en", game: null, message: "pack.toml: line 2: expected a value", from_folder: false },
    { place: "new-game/en", game: "new-game", message: "a game this version of ventris does not know", from_folder: false },
  ],
};

/** Which listing a test gives, and what the page is told about it, for either tab. */
function fromListing(given: Listing | null | undefined) {
  const listing = given === undefined ? withFaults : given;
  const own = listing?.own.find((folder) => folder.game === game.id);
  const packs = [...(own?.packs ?? []), ...(listing?.packs.filter((pack) => pack.game === game.id) ?? [])];
  return { listing, own, packs };
}

/** The game's page on the Play a translation tab. */
function play(props: {
  file?: GameFile;
  listing?: Listing | null;
  fetching?: boolean;
  fetchTrouble?: string | null;
  onLookAgain?: () => void;
  onMake?: () => void;
}) {
  const { listing, own, packs } = fromListing(props.listing);
  return render(
    <GamePage
      game={game}
      games={[game, other]}
      packs={packs}
      own={own}
      listing={listing}
      fetching={props.fetching ?? false}
      fetchTrouble={props.fetchTrouble ?? null}
      file={props.file}
      gamesFolder="C:\\games"
      searching={false}
      building={false}
      onFile={() => {}}
      onForget={() => {}}
      onChooseGamesFolder={() => {}}
      onLookAgain={props.onLookAgain ?? (() => {})}
      onBuild={() => {}}
      onMake={props.onMake ?? (() => {})}
    />,
  );
}

/** The game's page on the Make a translation tab. */
function make(props: {
  file?: GameFile;
  listing?: Listing | null;
  onSaveScript?: () => void;
  onBuild?: (pack: Pack) => void;
  listWith?: (work: () => Promise<Listing>) => Promise<void>;
}) {
  const { own } = fromListing(props.listing);
  return render(
    <TranslatePage
      game={game}
      games={[game, other]}
      own={own}
      file={props.file}
      gamesFolder="C:\\games"
      searching={false}
      building={false}
      onFile={() => {}}
      onForget={() => {}}
      onChooseGamesFolder={() => {}}
      onSaveScript={props.onSaveScript ?? (() => {})}
      onBuild={props.onBuild ?? (() => {})}
      listWith={props.listWith ?? (async (work) => void (await work()))}
    />,
  );
}

/** A `listWith` that runs the command at once and hands its listing to `shown`. */
const listingTo = (shown: (listing: Listing) => void) => async (work: () => Promise<Listing>) => shown(await work());

beforeEach(() => vi.resetAllMocks());
afterEach(cleanup);

describe("a game's page on the Play a translation tab", () => {
  it("has no script to save and no folder to choose, and points to the other tab for a pack of your own", () => {
    const onMake = vi.fn();
    play({ file, listing: { ...empty, fetched: true, checked: 1, packs: [fetchedPack] }, onMake });
    expect(screen.queryByText("Save the game's script…")).toBeNull();
    expect(screen.queryByText("Choose your pack's folder…")).toBeNull();
    expect(screen.getByText(/A pack of your own for this game\? Choose its folder in/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Make a translation" }));
    expect(onMake).toHaveBeenCalledTimes(1);
  });

  it("shows a pack that cannot be used with its reason, on its game's page", () => {
    play({ file });
    expect(screen.queryByText(/There is no translation for this game/)).toBeNull();
    // In the folder chosen for this game: one with a mistake, and one whose game cannot be told.
    expect(screen.getByText("999 is not a part of this game's script")).toBeTruthy();
    expect(screen.getByText("pack.toml: line 1: missing field `language`")).toBeTruthy();
    expect(
      screen.getByText("This pack has a mistake in it. Correct it, then choose Look again under Your pack, in Make a translation."),
    ).toBeTruthy();
    // A fetched one whose game cannot be told is shown on every page.
    expect(screen.getByText("pack.toml: line 2: expected a value")).toBeTruthy();
    // The folder chosen for another game is that game's, and not shown here.
    expect(screen.queryByText("string 06816.9999 does not exist")).toBeNull();
    expect(screen.getAllByText("Cannot be used")).toHaveLength(3);
    expect(screen.getAllByText("From your computer")).toHaveLength(2);
    // A fetched one for a game this version does not know has no page, and is listed below.
    expect(screen.getByText("1 translation from the ventris packs cannot be used by this version of ventris")).toBeTruthy();
    expect(screen.getByText(/Your own pack for this game is chosen in/)).toBeTruthy();
  });

  it("shows the limits of each body of text", () => {
    play({ file });
    expect(screen.getByText(game.kinds[0].limits)).toBeTruthy();
  });

  it("ends the table with the characters' names in a game whose pack can rename them, and only then", () => {
    const rows = () =>
      screen.getAllByRole("row").map((row) => Array.from(row.querySelectorAll("td")).map((cell) => cell.textContent));
    render(<WhatItCanChange game={game} />);
    expect(rows()[rows().length - 1]).toEqual([
      "characters' names",
      "rename them, in its names.txt file",
      "a name is 1 to 15 characters, without tags",
    ]);
    expect(rows()).toHaveLength(1 + game.kinds.length + 1);
    expect(screen.queryByText(/rename the game's characters/)).toBeNull();
    cleanup();

    render(<WhatItCanChange game={{ ...game, sets_names: false }} />);
    expect(screen.queryByText("characters' names")).toBeNull();
    expect(screen.queryByText(/names\.txt/)).toBeNull();
    expect(rows()).toHaveLength(1 + game.kinds.length);
  });
});

describe("the translations on the Play a translation tab", () => {
  it("says they are being got on the first start, when there is no copy yet", () => {
    play({ listing: empty, fetching: true });
    expect(screen.getByText("Getting the translations from the ventris packs on GitHub…")).toBeTruthy();
    // Before the first read of what is on this computer, the same.
    cleanup();
    play({ listing: null, fetching: false });
    expect(screen.getByText("Getting the translations from the ventris packs on GitHub…")).toBeTruthy();
  });

  it("says why when they could not be got, and points to Your pack on the other tab", () => {
    const onLookAgain = vi.fn();
    const onMake = vi.fn();
    play({ listing: empty, fetchTrouble: "github.com answered 404 Not Found", onLookAgain, onMake });
    expect(
      screen.getByText("The translations could not be got from the ventris packs on GitHub: github.com answered 404 Not Found."),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Look again" }));
    expect(onLookAgain).toHaveBeenCalledTimes(1);
    // GitHub answered, so this computer is online: that is not what to check.
    const advice = screen.getByText(/A pack of your own can be tried instead/);
    expect(advice.textContent).toBe(
      "Choose Look again to try once more. A pack of your own can be tried instead, under Your pack in Make a translation.",
    );
    expect(screen.queryByText(/online/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Make a translation" }));
    expect(onMake).toHaveBeenCalledTimes(1);
    cleanup();

    play({ listing: empty, fetchTrouble: "github.com could not be reached" });
    expect(screen.getByText(/A pack of your own can be tried instead/).textContent).toBe(
      "Check that this computer is online, then choose Look again. A pack of your own can be tried instead, under Your pack in Make a translation.",
    );
  });

  it("lists the fetched packs with no label, and the user's own first with one", () => {
    play({ file, listing: { ...empty, fetched: true, checked: 1, packs: [fetchedPack] } });
    expect(screen.getByText("Tales of Phantasia in English")).toBeTruthy();
    expect(screen.getByText("Machine translated and not yet reviewed by a human translator.")).toBeTruthy();
    expect(screen.queryByText("From your computer")).toBeNull();
    cleanup();

    const own = { ...ownFolder, packs: [ownPack] };
    play({ file, listing: { ...empty, own: [own], fetched: true, checked: 1, packs: [fetchedPack] } });
    const names = screen.getAllByText(/^(Mine|Tales of Phantasia in English)$/).map((element) => element.textContent);
    expect(names).toEqual(["Mine", "Tales of Phantasia in English"]);
    expect(screen.getAllByText("From your computer")).toHaveLength(1);
    expect(screen.getAllByRole("button", { name: "Build the game…" })).toHaveLength(2);
  });

  it("says when there is no translation for the game", () => {
    play({ listing: { ...empty, fetched: true, checked: 1 } });
    expect(screen.getByText("There is no translation for this game in the ventris packs yet.")).toBeTruthy();
  });
});

describe("a game's page on the Make a translation tab", () => {
  const fetchedOnly: Listing = { ...empty, fetched: true, checked: 1, packs: [fetchedPack] };

  it("has the steps, and offers to save the game's script once the game's file is found", () => {
    const onSaveScript = vi.fn();
    make({ listing: fetchedOnly, onSaveScript });
    const steps = screen.getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent);
    expect(steps).toEqual([
      "1Your copy of the game",
      "2Save the game's script",
      "3Your pack",
      "4Build the game to try it",
    ]);
    expect(screen.getByRole("button", { name: "Translating a game" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Save the game's script…" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByText("It can be saved once your copy of the game is found.")).toBeTruthy();
    expect(screen.getByText("What a translation can change in this game")).toBeTruthy();
    // The fetched packs are not the translator's, and are not offered here.
    expect(screen.queryByText("Tales of Phantasia in English")).toBeNull();
    cleanup();

    make({ file, listing: fetchedOnly, onSaveScript });
    fireEvent.click(screen.getByRole("button", { name: "Save the game's script…" }));
    expect(onSaveScript).toHaveBeenCalledTimes(1);
    expect(screen.getByText(/never put it in a pack or share it/)).toBeTruthy();
    expect(screen.getByText("Once your pack is chosen and can be used, build the game from it here.")).toBeTruthy();
    expect(screen.getByText(/Building is what checks your text against the game itself/)).toBeTruthy();
  });

  it("offers to choose your pack's folder, and takes it for this game", async () => {
    const onListing = vi.fn();
    const after: Listing = { ...fetchedOnly, own: [{ ...ownFolder, packs: [ownPack] }] };
    vi.mocked(backend.pickFolder).mockResolvedValue("C:\\mine");
    vi.mocked(backend.chooseOwnPacks).mockResolvedValue(after);
    make({ file, listing: fetchedOnly, listWith: listingTo(onListing) });
    expect(screen.getByText(/Choose the folder your pack for this game is in: one you are writing/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Choose your pack's folder…" }));
    await waitFor(() => expect(onListing).toHaveBeenCalledWith(after));
    expect(backend.chooseOwnPacks).toHaveBeenCalledWith("top-sfc", "C:\\mine");
  });

  it("says why a folder with no pack for this game is not taken", async () => {
    const onListing = vi.fn();
    const why = "There is no pack for Tales of Phantasia in C:\\theirs. The pack in it is for Tales of Destiny 2.";
    vi.mocked(backend.pickFolder).mockResolvedValue("C:\\theirs");
    vi.mocked(backend.chooseOwnPacks).mockRejectedValue(why);
    make({ file, listing: fetchedOnly, listWith: listingTo(onListing) });
    fireEvent.click(screen.getByRole("button", { name: "Choose your pack's folder…" }));
    expect((await screen.findByRole("status")).textContent).toBe(why);
    expect(onListing).not.toHaveBeenCalled();
    // Nothing is taken, so the way to choose a folder is still there.
    expect(screen.getByRole("button", { name: "Choose your pack's folder…" })).toBeTruthy();
  });

  it("shows the folder and the pack once chosen, builds from it, reads it again, and stops using it", async () => {
    const onListing = vi.fn();
    const onBuild = vi.fn();
    vi.mocked(backend.listPacks).mockResolvedValue(fetchedOnly);
    vi.mocked(backend.forgetOwnPacks).mockResolvedValue(fetchedOnly);
    make({ file, listing: { ...fetchedOnly, own: [{ ...ownFolder, packs: [ownPack] }] }, listWith: listingTo(onListing), onBuild });
    expect(screen.getByText("Your pack for this game is read from this folder:")).toBeTruthy();
    expect(screen.getByText("C:\\mine")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Choose your pack's folder…" })).toBeNull();
    expect(screen.getByRole("button", { name: "Change" })).toBeTruthy();
    // The pack as the app reads it, marked as the user's, and built from in step 4.
    expect(screen.getAllByText("Mine")).toHaveLength(2);
    expect(screen.getByText("From your computer")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Build the game…" }));
    expect(onBuild).toHaveBeenCalledWith(ownPack);
    // Look again reads the folder again, and fetches nothing.
    fireEvent.click(screen.getByRole("button", { name: "Look again" }));
    await waitFor(() => expect(backend.listPacks).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole("button", { name: "Stop using it" }));
    await waitFor(() => expect(backend.forgetOwnPacks).toHaveBeenCalledWith("top-sfc"));
    expect(onListing).toHaveBeenCalledTimes(2);
  });

  it("shows a pack that cannot be used with its reason", () => {
    make({ file });
    expect(screen.getByText("999 is not a part of this game's script")).toBeTruthy();
    expect(screen.getByText("This pack has a mistake in it. Correct it, then choose Look again.")).toBeTruthy();
    expect(
      screen.getByText("Its pack.toml cannot be read far enough to say which game it is for. Correct it, then choose Look again."),
    ).toBeTruthy();
    // Only the folder chosen for this game: neither another game's nor a fetched pack's faults.
    expect(screen.getAllByText("Cannot be used")).toHaveLength(2);
    expect(screen.getByText("Once your pack is chosen and can be used, build the game from it here.")).toBeTruthy();
  });

  it("says when the folder has gone, or holds no pack for this game any more", () => {
    make({ file, listing: { ...fetchedOnly, own: [{ ...ownFolder, trouble: "It is no longer there." }] } });
    expect(screen.getByText("It is no longer there. The ventris packs are still listed.")).toBeTruthy();
    cleanup();

    make({ file, listing: { ...fetchedOnly, own: [ownFolder] } });
    expect(screen.getByText("There is no pack for this game in it now.")).toBeTruthy();
  });
});

describe("where the translations come from", () => {
  const kept: Listing = { ...empty, fetched: true, checked: new Date(2026, 10, 23, 17, 20).getTime() / 1000 };
  const note = (listing: Listing | null, fetching: boolean, trouble: string | null) =>
    translationsNote({ listing, fetching, trouble }).words.replace(/\s/g, " ");

  it("says how up to date the copy is, and what went wrong", () => {
    expect(note(empty, true, null)).toBe("Getting the translations…");
    expect(note(kept, true, null)).toBe("Looking for new translations…");
    expect(note(kept, false, null)).toBe("Up to date as of Nov 23, 2026, 5:20 PM");
    expect(note(kept, false, "github.com could not be reached (host not found)")).toBe(
      "The translations could not be refreshed: github.com could not be reached (host not found). The ones shown are from Nov 23, 2026, 5:20 PM.",
    );
    expect(note(empty, false, "github.com answered 404 Not Found")).toBe(
      "The translations could not be got: github.com answered 404 Not Found.",
    );
    expect(note(null, false, null)).toBe("None yet.");
  });

  it("tells an answer from the server apart from not reaching it", () => {
    expect(answeredWithError("github.com answered 404 Not Found")).toBe(true);
    expect(answeredWithError("127.0.0.1:8765 answered 500 Internal Server Error")).toBe(true);
    expect(answeredWithError("github.com could not be reached")).toBe(false);
    expect(answeredWithError("github.com took too long to answer")).toBe(false);
  });

  it("fetches again on Look again, but not while a fetch is going", () => {
    const onLookAgain = vi.fn();
    render(<Translations listing={kept} fetching={false} trouble={null} onLookAgain={onLookAgain} />);
    expect(screen.getByText("From the ventris packs on GitHub")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Look again" }));
    expect(onLookAgain).toHaveBeenCalledTimes(1);
    cleanup();
    render(<Translations listing={kept} fetching={true} trouble={null} onLookAgain={onLookAgain} />);
    expect(screen.getByRole("button", { name: "Look again" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("status").textContent).toBe("Looking for new translations…");
  });
});

describe("saving a game's script", () => {
  it("says where the script was written, and why when it was not", () => {
    const saved = { folder: "C:\\work\\Tales of Phantasia script", files: 52, lines: ["game:      Tales of Phantasia"] };
    render(<ScriptDialog job={{ game, state: "done", saved }} onClose={() => {}} />);
    expect(screen.getByText("The game's script is saved")).toBeTruthy();
    expect(screen.getByText(saved.folder)).toBeTruthy();
    expect(screen.getByText("Tales of Phantasia, in 52 text files")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Show in folder" })).toBeTruthy();
    cleanup();

    const message = "C:\\work\\Tales of Phantasia script is already there and is not empty, so nothing was written.";
    render(<ScriptDialog job={{ game, state: "refused", folder: "C:\\work", message }} onClose={() => {}} />);
    expect(screen.getByRole("alert").textContent).toBe(message);
  });
});
