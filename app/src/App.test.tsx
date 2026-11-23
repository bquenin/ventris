import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import App from "./App";
import * as backend from "./lib/backend";
import type { Game, Listing, OwnFolder } from "./lib/backend";

const phantasia: Game = {
  id: "top-sfc",
  name: "Tales of Phantasia (Super Famicom, Japan)",
  platform: "Super Famicom / SNES",
  source_language: "ja",
  releases: [{ sha1: "0000000000000000000000000000000000000001", name: "Japan" }],
  kinds: [],
  can_insert: true,
  sets_names: true,
  name_limits: "",
};
const destiny: Game = { ...phantasia, id: "tod2-ps2", name: "Tales of Destiny 2 (PlayStation 2, Japan)", platform: "PlayStation 2" };
const listing: Listing = { own: [], fetched: true, checked: 1, packs: [], faults: [] };
const mine: OwnFolder = { game: "tod2-ps2", folder: "C:\\mine", trouble: null, packs: [], faults: [] };
/** What the program would list now: what the commands below last changed. */
let now: Listing = listing;

// The commands behind the window, answering at once and with no network.
vi.mock("./lib/backend", async (original) => ({
  ...(await original<typeof import("./lib/backend")>()),
  listGames: vi.fn(async () => [phantasia, destiny]),
  remembered: vi.fn(async () => ({ games_folder: null, own_packs: {}, games: {} })),
  listPacks: vi.fn(async () => now),
  refreshPacks: vi.fn(async () => listing),
  pickFolder: vi.fn(async () => "C:\\mine"),
  chooseOwnPacks: vi.fn(async () => {
    now = { ...listing, own: [mine] };
    return now;
  }),
}));

afterEach(() => {
  cleanup();
  now = listing;
});

describe("the window", () => {
  it("keeps the list of games on both tabs, and the game chosen when the tab changes", async () => {
    render(<App />);
    const sidebar = await screen.findByRole("complementary");
    fireEvent.click(await within(sidebar).findByText("Tales of Destiny 2"));
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Tales of Destiny 2");
    expect(screen.queryByRole("button", { name: "Save the game's script…" })).toBeNull();

    fireEvent.click(within(screen.getByRole("navigation")).getByRole("button", { name: "Make a translation" }));
    expect(screen.getByRole("complementary")).toBeTruthy();
    expect(within(screen.getByRole("complementary")).getByText("Translations")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Tales of Destiny 2");
    expect(screen.getByRole("button", { name: "Save the game's script…" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Choose your pack's folder…" })).toBeTruthy();

    // Another game chosen on this tab stays chosen back on the other.
    fireEvent.click(within(screen.getByRole("complementary")).getByText("Tales of Phantasia"));
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Tales of Phantasia");
    fireEvent.click(within(screen.getByRole("navigation")).getByRole("button", { name: "Play a translation" }));
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Tales of Phantasia");
    expect(screen.getByRole("heading", { name: /Choose a translation/ })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Save the game's script…" })).toBeNull();
  });

  it("does not let a fetch that ends late put back the packs as they were before a folder was chosen", async () => {
    // The fetch's own listing was read before the folder was chosen, and arrives after.
    let finish: (listed: Listing) => void = () => {};
    vi.mocked(backend.refreshPacks).mockImplementationOnce(() => new Promise((resolve) => (finish = resolve)));
    render(<App />);
    fireEvent.click(await within(await screen.findByRole("complementary")).findByText("Tales of Destiny 2"));
    fireEvent.click(within(screen.getByRole("navigation")).getByRole("button", { name: "Make a translation" }));
    fireEvent.click(screen.getByRole("button", { name: "Choose your pack's folder…" }));
    expect(await screen.findByText("Your pack for this game is read from this folder:")).toBeTruthy();

    vi.mocked(backend.listPacks).mockClear();
    finish(listing);
    // The packs are listed again after it, and the folder chosen is still there.
    await waitFor(() => expect(backend.listPacks).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.queryByText("Looking for new translations…")).toBeNull());
    expect(screen.getByText("Your pack for this game is read from this folder:")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Choose your pack's folder…" })).toBeNull();
  });

  it("shows a fetch's own listing when nothing else listed the packs while it ran", async () => {
    vi.mocked(backend.listPacks).mockClear();
    render(<App />);
    await screen.findByRole("complementary");
    await waitFor(() => expect(backend.refreshPacks).toHaveBeenCalled());
    await waitFor(() => expect(screen.queryByText(/Looking for new translations|Getting the translations/)).toBeNull());
    // Read once at the start, and not again after the fetch.
    expect(backend.listPacks).toHaveBeenCalledTimes(1);
  });
});