// What the window asks of the program behind it. Each function is one command in
// src-tauri/src/lib.rs, and the types here are the shapes that file gives back.
//
// Opened in a browser with `npm run dev`, there is no program behind the page: made-up answers
// from ./mock stand in, so that the window can be worked on without building the app.

import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";

import { mock } from "./mock";

/** One body of text of a game, and what a pack can do with it. */
export interface Kind {
  name: string;
  insert: boolean;
  grow: boolean;
  new_characters: boolean;
  max_length: number | null;
  limits: string;
}

/** A release of a game that ventris reads, in full, so that a player can check a file against it. */
export interface Release {
  sha1: string;
  /** "Japan, version 1.01" */
  name: string;
  /** Its size in bytes. */
  size: number;
  /** What the image says it is: a disc's serial, "SLPS-25172", or a cartridge's title. */
  label: string;
  version: string;
  /** What the label is, as a word to show before it: "serial", "title". */
  label_kind: string;
  /** Where a catalogue of dumps lists it, when one does. */
  catalogue: Catalogued | null;
}

/** A release as a catalogue of dumps lists it. The catalogues host no games: a record is facts about a dump. */
export interface Catalogued {
  /** "Redump" */
  name: string;
  /** What it is, for someone who has never heard of it: "the catalogue of disc dumps". */
  about: string;
  /** The release's name there: "Tales of Destiny 2 (Japan, Asia)". */
  entry: string;
  /** The address of its record there, opened in the browser; null where none is given. */
  page: string | null;
}

export interface Game {
  /** The id of the game's adapter, which packs name: "tod2-ps2". */
  id: string;
  /** "Tales of Destiny 2 (PlayStation 2, Japan)" */
  name: string;
  platform: string;
  /** The language of the game's own text, as a tag: "ja". */
  source_language: string;
  /** The releases ventris reads, which are the only files of the game it takes. */
  releases: Release[];
  kinds: Kind[];
  can_insert: boolean;
  sets_names: boolean;
  /** What limits a name a pack gives the characters, in a phrase like a kind's limits; empty when nothing is said. */
  name_limits: string;
}

export interface Pack {
  /** Where the pack is, for building from it. */
  folder: string;
  /** The same from the folder it was found in, for showing: "tod2-ps2/en". */
  place: string;
  game: string;
  language: string;
  name: string;
  version: string;
  authors: string[];
  license: string | null;
  description: string | null;
  homepage: string | null;
  changes: { kind: string; strings: number }[];
  names: number;
  /** Whether it is in a folder the user chose for its game, and not one of the packs fetched from GitHub. */
  from_folder: boolean;
}

/** A folder that has a pack.toml and cannot be built from, and why. */
export interface Fault {
  /** Where it is from the folder it was found in: "top-sfc/fr". */
  place: string;
  /** The game it is for, or null when its pack.toml cannot be read far enough to say. */
  game: string | null;
  /** Why, in the words `ventris describe` gives. */
  message: string;
  /** Whether it is in a folder the user chose for its game, and not one of the packs fetched from GitHub. */
  from_folder: boolean;
}

/**
 * A folder of packs of the user's own for one game, chosen on that game's Make a translation tab: a pack they are
 * writing, or one they got elsewhere. Only that game's packs are taken from it.
 */
export interface OwnFolder {
  /** The id of the game it was chosen for. */
  game: string;
  folder: string;
  /** Why the folder could not be read, as a sentence to show under it: "It is no longer there." */
  trouble: string | null;
  packs: Pack[];
  /** Its faulty packs for that game, and those whose pack.toml cannot be read far enough to say which game. */
  faults: Fault[];
}

/** Every pack there is on this computer: the folders the user chose for their games, and the copy fetched from GitHub. */
export interface Listing {
  /** One folder for each game the user chose one for. */
  own: OwnFolder[];
  /** Whether there is a copy of the packs fetched from GitHub. */
  fetched: boolean;
  /** When that copy was last found to be up to date, in seconds since 1970; 0 when not known. */
  checked: number;
  /** The packs of that copy, for every game. */
  packs: Pack[];
  faults: Fault[];
}

/** A game's own file: one found in the games folder, or one the user chose. */
export interface GameFile {
  path: string;
  release: string;
  sha1: string;
  /** Whether the user chose this file themselves, and it was not found by a search. */
  chosen: boolean;
}

export interface Remembered {
  /** The folder the user keeps their games in, which is searched. */
  games_folder: string | null;
  /** The folder of a pack of the user's own, by the id of the game it was chosen for. */
  own_packs: Record<string, string>;
  /** Each game's file, by the id of the game's adapter. */
  games: Record<string, GameFile>;
  /**
   * Given by a search of the games folder, and not remembered: for each game not found there, what the folder holds of
   * it, as sentences that start with a file's path from the folder (a wrong copy and why it is not read, compressed
   * disc images). A game with nothing to say is left out.
   */
  near?: Record<string, string[]>;
}

export interface Recognised {
  game: string;
  file: GameFile;
}

export interface Built {
  output: string;
  lines: string[];
  sha1: string;
  identical: boolean;
}

/** What saving a game's script wrote. */
export interface Saved {
  /** The folder the script was written to, inside the one the user chose. */
  folder: string;
  files: number;
  lines: string[];
}

export interface About {
  version: string;
  fonts: { name: string; about: string; license: string; terms: string }[];
}

/** Whether the page is inside the app, and not opened in a browser to work on it. */
export function insideApp(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function listGames(): Promise<Game[]> {
  return insideApp() ? invoke("games") : mock.games();
}

export function remembered(): Promise<Remembered> {
  return insideApp() ? invoke("remembered") : mock.remembered();
}

/** Look through the games folder again, and give what is known after it. */
export function searchGames(): Promise<Remembered> {
  return insideApp() ? invoke("search_games") : mock.searchGames();
}

/** Take a folder as the place the user keeps their games, and search it. */
export function chooseGamesFolder(folder: string): Promise<Remembered> {
  return insideApp() ? invoke("choose_games_folder", { folder }) : mock.chooseGamesFolder(folder);
}

/** The packs on this computer now, from the user's own folders and the copy fetched before. Uses no network. */
export function listPacks(): Promise<Listing> {
  return insideApp() ? invoke("packs") : mock.packs();
}

/**
 * Fetch the ventris packs from GitHub, keep them, and give every pack there is after it. Fails, in words to show,
 * when they could not be fetched; the copy kept before is then still there.
 */
export function refreshPacks(): Promise<Listing> {
  return insideApp() ? invoke("refresh_packs") : mock.refreshPacks();
}

/**
 * Take a folder as the one a game's own packs are read from, and give every pack there is after it. Fails, in words to
 * show, when it holds no pack for that game; it is then not remembered.
 */
export function chooseOwnPacks(game: string, folder: string): Promise<Listing> {
  return insideApp() ? invoke("choose_own_packs", { game, folder }) : mock.chooseOwnPacks(game, folder);
}

/** Stop reading a game's own packs from the folder chosen for it. */
export function forgetOwnPacks(game: string): Promise<Listing> {
  return insideApp() ? invoke("forget_own_packs", { game }) : mock.forgetOwnPacks(game);
}

/** Say which game a file the user chose is. It is remembered as that game's file. */
export function recogniseGame(path: string): Promise<Recognised> {
  return insideApp() ? invoke("recognise_game", { path }) : mock.recogniseGame(path);
}

/** Forget the file the user chose for a game, and go back to what the games folder holds. */
export function forgetGame(game: string): Promise<Remembered> {
  return insideApp() ? invoke("forget_game", { game }) : mock.searchGames();
}

export function suggestOutput(game: string, pack: string): Promise<string> {
  return insideApp() ? invoke("suggest_output", { game, pack }) : mock.suggestOutput(game);
}

export function build(game: string, pack: string, output: string): Promise<Built> {
  return insideApp() ? invoke("build", { game, pack, output }) : mock.build(output);
}

/** Save a game's own script, to translate from, in a folder of its own inside the one chosen. */
export function saveScript(game: string, file: string, folder: string): Promise<Saved> {
  return insideApp() ? invoke("save_script", { game, file, folder }) : mock.saveScript(game, folder);
}

export function about(): Promise<About> {
  return insideApp() ? invoke("about") : mock.about();
}

/** Ask for a folder. Null when the user closes the dialog without choosing. */
export async function pickFolder(title: string): Promise<string | null> {
  if (!insideApp()) return "C:\\Users\\you\\a-folder";
  return open({ title, directory: true, multiple: false });
}

/** Ask for the game's own file. */
export async function pickGameFile(title: string): Promise<string | null> {
  if (!insideApp()) return "C:\\Users\\you\\games\\game.iso";
  return open({ title, directory: false, multiple: false });
}

/** Ask where to write the built game, offering a name. */
export async function pickOutput(suggested: string): Promise<string | null> {
  if (!insideApp()) return suggested;
  const extension = /\.([A-Za-z0-9]+)$/.exec(suggested)?.[1];
  const filters = extension ? [{ name: "Game file", extensions: [extension] }] : [];
  return save({ title: "Save the translated game as", defaultPath: suggested, filters });
}

export async function showInFolder(path: string): Promise<void> {
  if (insideApp()) await revealItemInDir(path);
}

/**
 * Open a web address in the browser: a pack's home page, or the translators' guide. A pack comes from anyone,
 * so only a web address is opened.
 */
export async function openHomepage(address: string): Promise<void> {
  if (!/^https?:\/\//i.test(address)) return;
  if (insideApp()) await openUrl(address);
  else window.open(address, "_blank", "noopener");
}
