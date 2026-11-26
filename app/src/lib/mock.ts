// Made-up answers for when the page is opened in a browser, with no program behind it.
// Nothing here is used inside the app.

import type { About, Built, Game, Listing, OwnFolder, Recognised, Remembered, Saved } from "./backend";

const games: Game[] = [
  {
    id: "top-sfc",
    name: "Tales of Phantasia (Super Famicom, Japan)",
    platform: "Super Famicom / SNES",
    source_language: "ja",
    releases: [
      {
        sha1: "fdecb234f416eb1cc1924e4f3d52c080c5bcd8d0",
        name: "Japan",
        size: 6291456,
        label: "TALES OF PHANTASIA",
        version: "1.0",
        label_kind: "title",
        catalogue: { name: "No-Intro", about: "the catalogue of cartridge dumps", entry: "Tales of Phantasia (Japan)", page: null },
      },
    ],
    kinds: [
      {
        name: "dialogue",
        insert: true,
        grow: true,
        new_characters: true,
        max_length: null,
        limits: "an area's longest string and its events must fit together in the 16,480 bytes of memory the game has for them",
      },
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
    name_limits:
      "a name is 1 to 15 characters, without tags; the menus hold names of up to 7, and keep the game's own names when one is longer",
  },
  {
    id: "tod2-ps2",
    name: "Tales of Destiny 2 (PlayStation 2, Japan)",
    platform: "PlayStation 2",
    source_language: "ja",
    releases: [
      {
        sha1: "3b2be72b751607a7a2a56d62feec71584b4b8c72",
        name: "Japan, version 1.01",
        size: 3234004992,
        label: "SLPS-25172",
        version: "1.01",
        label_kind: "serial",
        catalogue: {
          name: "Redump",
          about: "the catalogue of disc dumps",
          entry: "Tales of Destiny 2 (Japan, Asia)",
          page: "http://redump.org/disc/15921/",
        },
      },
    ],
    kinds: [
      { name: "dialogue", insert: true, grow: true, new_characters: true, max_length: null, limits: "" },
      { name: "skits", insert: true, grow: true, new_characters: true, max_length: null, limits: "" },
      { name: "menus", insert: true, grow: true, new_characters: true, max_length: null, limits: "" },
      { name: "lists", insert: true, grow: true, new_characters: true, max_length: null, limits: "" },
      { name: "enemies", insert: true, grow: false, new_characters: true, max_length: null, limits: "a name takes at most 22 bytes" },
      { name: "minigames", insert: true, grow: false, new_characters: true, max_length: null, limits: "a string takes no more bytes than the game's own" },
    ],
    can_insert: true,
    sets_names: false,
    name_limits: "",
  },
];

/** Where the app keeps its copy of the ventris packs fetched from GitHub. */
const kept = "C:\\Users\\you\\AppData\\Local\\io.github.bquenin.ventris\\packs\\copy-1";

/** The packs as fetched from GitHub. */
const fetched: Listing = {
  own: [],
  fetched: true,
  checked: 1795467600,
  packs: [
    {
      folder: `${kept}\\top-sfc\\en`,
      place: "top-sfc/en",
      game: "top-sfc",
      language: "en",
      name: "Tales of Phantasia in English",
      version: "0.9",
      authors: ["Machine translated, directed by bquenin"],
      license: "CC-BY-NC-SA-4.0",
      description: "Machine translated and not yet reviewed by a human translator.",
      homepage: null,
      changes: [
        { kind: "dialogue", strings: 7716 },
        { kind: "menu", strings: 1290 },
      ],
      names: 9,
      from_folder: false,
    },
    {
      folder: `${kept}\\tod2-ps2\\en`,
      place: "tod2-ps2/en",
      game: "tod2-ps2",
      language: "en",
      name: "Tales of Destiny 2 in English",
      version: "0.1",
      authors: ["Machine translated, directed by bquenin"],
      license: "CC-BY-NC-SA-4.0",
      description:
        "Machine translated and not yet reviewed by a human translator. A first full pass. The dialogue of every story script from the prologue to the ending, 608 skits, the menu screens, the messages of the battle program, the enemy names, the lists of artes, items, titles, recipes, shops and places, and the quiz books and minigames. It builds with nothing refused. On screen, the opening, ten skits, the menus, a shop, the first battle and a quiz book have been seen; nothing past Cresta has been. NOTES.md has the names and terms chosen and what is still open.",
      homepage: null,
      changes: [
        { kind: "dialogue", strings: 28816 },
        { kind: "skits", strings: 2411 },
        { kind: "menus", strings: 707 },
        { kind: "lists", strings: 3249 },
        { kind: "enemies", strings: 933 },
        { kind: "minigames", strings: 5230 },
      ],
      names: 0,
      from_folder: false,
    },
  ],
  faults: [],
};

const own = "C:\\Users\\you\\my-pack";

/**
 * A folder of the user's own for Tales of Destiny 2, with a pack they are writing and two that have a mistake in them.
 * Chosen on that game's Make a translation tab, it is taken; chosen for another game, it is refused, as the app does.
 */
const yours: Pick<OwnFolder, "packs" | "faults"> = {
  packs: [
    {
      folder: `${own}\\pt-BR`,
      place: "pt-BR",
      game: "tod2-ps2",
      language: "pt-BR",
      name: "Tales of Destiny 2 em português",
      version: "0.3",
      authors: ["Ana", "Rui"],
      license: "CC-BY-NC-SA-4.0",
      description: "The story and the skits. Menus are not done yet.",
      homepage: "https://example.org/my-pack",
      changes: [
        { kind: "dialogue", strings: 18204 },
        { kind: "skits", strings: 3120 },
      ],
      names: 0,
      from_folder: true,
    },
  ],
  faults: [
    {
      place: "fr",
      game: "tod2-ps2",
      message: "06816.txt: '[06816.4136' has one of its brackets missing: a string's header is written '[06816.4136]'",
      from_folder: true,
    },
    { place: "de", game: null, message: "pack.toml: line 1: missing field `language`", from_folder: true },
  ],
};

/**
 * Opened in a browser, the page starts with no copy of the packs and gets them after a moment, as on a first start.
 * With `?offline` in the address the fetch fails, to see how that is shown.
 */
const offline = typeof window !== "undefined" && window.location.search.includes("offline");
/** The folder the user chose for a game's own packs, by game id. */
const ownPacks: Record<string, string> = {};
let gotThem = false;

/** What the app lists: the folders chosen for games, and the fetched packs once there are some. */
function listing(): Listing {
  const theirs = gotThem ? fetched : { ...fetched, fetched: false, checked: 0, packs: [], faults: [] };
  const own = Object.entries(ownPacks).map(([game, folder]) => ({ game, folder, trouble: null, ...yours }));
  return { ...theirs, own };
}

/**
 * With `?missing` in the address the games folder holds neither game, to see the page that says so; with `?near` as
 * well, it holds wrong copies of both, which the page names; with `?refuse`, a file chosen by hand is refused, as a disc
 * of another release is.
 */
const missing = typeof window !== "undefined" && window.location.search.includes("missing");
const near = typeof window !== "undefined" && window.location.search.includes("near");
const refuse = typeof window !== "undefined" && window.location.search.includes("refuse");

/** What the engine says of wrong copies in a games folder; the words are the engine's own, copied. */
const nearMisses: Record<string, string[]> = {
  "tod2-ps2": [
    "ps2\\Tales of Destiny 2 (Japan) (PlayStation 2 the Best).iso is Tales of Destiny 2, but not the release ventris reads: " +
      "it is the PlayStation 2 the Best reissue, in Japanese (serial SLPS-73219, version 2.01).",
    "ps2\\Tales of Destiny 2 (Japan).chd, ps2\\Ace Combat 5.cso and ps2\\Ico.chd are compressed disc images (CHD, CSO or " +
      "ZSO): ventris cannot read them, or tell which game they hold. If one is this game, convert it back to a plain disc " +
      "image first.",
  ],
  "top-sfc": [
    "snes\\Tales of Phantasia (Japan) [T-En DeJap].sfc is Tales of Phantasia with DeJap's English translation already " +
      'applied (title "ToP English (C) DeJap", version 1.0). ventris reads the game as it was released, so it needs a copy ' +
      "without the patch.",
  ],
};

/** What the engine says of a disc of another release; the words are the engine's own, copied. */
function refusal(path: string): string {
  return (
    `${path} is Tales of Destiny 2, but not the release ventris reads: it is the Mega Hits! reissue, in Japanese ` +
    '(serial SLPS-72502, version 2.01). The release ventris reads is Japan, version 1.01, named "Tales of Destiny 2 ' +
    '(Japan, Asia)" in Redump, the catalogue of disc dumps: serial SLPS-25172, 3,234,004,992 bytes, SHA-1 ' +
    "3b2be72b751607a7a2a56d62feec71584b4b8c72."
  );
}

/** What a search of a games folder gives: both games, one of them in an archive. */
function found(folder: string): Remembered {
  if (missing) return { games_folder: folder, own_packs: { ...ownPacks }, games: {}, near: near ? nearMisses : {} };
  return {
    games_folder: folder,
    own_packs: { ...ownPacks },
    games: {
      "top-sfc": {
        path: `${folder}\\snes\\Tales of Phantasia (Japan).zip`,
        release: "Japan",
        sha1: games[0].releases[0].sha1,
        chosen: false,
      },
      "tod2-ps2": {
        path: `${folder}\\ps2\\Tales of Destiny 2 (Japan) (v1.01).iso`,
        release: "Japan, version 1.01",
        sha1: games[1].releases[0].sha1,
        chosen: false,
      },
    },
  };
}

function later<T>(value: T, milliseconds = 600): Promise<T> {
  return new Promise((resolve) => setTimeout(() => resolve(value), milliseconds));
}

export const mock = {
  games: (): Promise<Game[]> => later(games, 0),
  remembered: (): Promise<Remembered> => later({ games_folder: null, own_packs: { ...ownPacks }, games: {} }, 0),
  searchGames: (): Promise<Remembered> => later(found("C:\\Users\\you\\roms"), 900),
  chooseGamesFolder: (folder: string): Promise<Remembered> => later(found(folder), 900),
  packs: (): Promise<Listing> => later(listing(), 0),
  refreshPacks: (): Promise<Listing> =>
    offline
      ? new Promise((_, reject) => setTimeout(() => reject("github.com could not be reached"), 1500))
      : later(null, 1500).then(() => {
          gotThem = true;
          return listing();
        }),
  chooseOwnPacks: (game: string, folder: string): Promise<Listing> => {
    if (game !== "tod2-ps2") {
      const title = (games.find((listed) => listed.id === game)?.name ?? game).replace(/\s*\(.*$/, "");
      return new Promise((_, reject) =>
        setTimeout(() => reject(`There is no pack for ${title} in ${folder}. The pack in it is for Tales of Destiny 2.`), 300),
      );
    }
    ownPacks[game] = folder;
    return later(listing());
  },
  forgetOwnPacks: (game: string): Promise<Listing> => {
    delete ownPacks[game];
    return later(listing(), 0);
  },
  recogniseGame: (path: string): Promise<Recognised> =>
    refuse
      ? new Promise((_, reject) => setTimeout(() => reject(refusal(path)), 1200))
      : later(
      { game: "tod2-ps2", file: { path, release: "Japan, version 1.01", sha1: games[1].releases[0].sha1, chosen: true } },
      1200,
    ),
  suggestOutput: (game: string): Promise<string> => later(game.replace(/(\.[^.\\/]+)?$/, " (Tales of Destiny 2 in English 0.1, en)$1"), 0),
  build: (output: string): Promise<Built> =>
    later(
      {
        output,
        lines: [
          "dialogue:  28816 strings changed in 460 scripts, 9120 of them moved",
          "skits:     2411 strings changed in 608 skits, 1530 of them moved",
          "skit pace: 2411 changed lines timed, 1204 of them typed faster than the game's pace",
          "menus:     707 strings changed in 22 menus, 310 of them moved",
          "lists:     3249 strings changed, 2210 of them moved",
          "spells:    drawn in the narrow font in 2 grids of artes",
          "enemies:   933 names changed in 550 packs, and 933 on the battle's name plates",
          "minigames: 5230 strings changed in 90 scripts",
          "font:      74 new characters, 62 at half width and 12 at full width",
        ],
        sha1: "f7d06518e75a1c8745083541c7b86745fab2047b",
        identical: false,
      },
      2500,
    ),
  saveScript: (game: string, folder: string): Promise<Saved> => {
    const title = (games.find((listed) => listed.id === game)?.name ?? game).replace(/\s*\(.*$/, "");
    return later(
      {
        folder: `${folder}\\${title} script`,
        files: 1291,
        lines: [
          `game:      ${title}`,
          "dialogue:  39929 strings in 460 sections",
          "skits:     3022 strings in 608 sections",
          "menus:     716 strings in 22 sections",
          "unmapped:  0 characters using 0 codes that have no table entry yet",
        ],
      },
      2000,
    );
  },
  about: (): Promise<About> =>
    later(
      {
        version: "0.2.0",
        fonts: [
          { name: "unifont", about: "GNU Unifont 18.0.01", license: "OFL-1.1", terms: "SIL OPEN FONT LICENSE Version 1.1 …" },
          { name: "unscii-8", about: "Unscii-8, 8 by 8 pixels", license: "Public domain", terms: "Unscii is in the public domain." },
        ],
      },
      0,
    ),
};
