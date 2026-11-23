import { describe, expect, it } from "vitest";

import type { Pack } from "./backend";
import { counted, dateAndTime, fileName, languageName, splitName, whatItChanges } from "./format";

const pack: Pack = {
  folder: "packs/tod2-ps2/en",
  place: "tod2-ps2/en",
  game: "tod2-ps2",
  language: "en",
  name: "Trial",
  version: "0.1",
  authors: ["Ana"],
  license: null,
  description: null,
  homepage: null,
  changes: [],
  names: 0,
  from_folder: false,
};

describe("wording", () => {
  it("names a language by its tag, and shows a tag it has no name for", () => {
    expect(languageName("en")).toBe("English");
    expect(languageName("pt-BR")).toBe("Brazilian Portuguese");
    expect(languageName("ja")).toBe("Japanese");
    expect(languageName("not a tag")).toBe("not a tag");
  });

  it("splits a game's name into its title and its release", () => {
    expect(splitName("Tales of Destiny 2 (PlayStation 2, Japan)")).toEqual({
      title: "Tales of Destiny 2",
      release: "PlayStation 2, Japan",
    });
    expect(splitName("A game with no release")).toEqual({ title: "A game with no release", release: "" });
  });

  it("counts with the right noun", () => {
    expect(counted(1, "pack")).toBe("1 pack");
    expect(counted(0, "pack")).toBe("0 packs");
    expect(counted(18204, "string")).toBe("18,204 strings");
  });

  it("says what a pack changes", () => {
    expect(whatItChanges(pack)).toBe("0 strings");
    expect(whatItChanges({ ...pack, changes: [{ kind: "menus", strings: 6 }] })).toBe("6 strings in menus");
    expect(
      whatItChanges({
        ...pack,
        changes: [
          { kind: "dialogue", strings: 18204 },
          { kind: "skits", strings: 3120 },
        ],
        names: 1,
      }),
    ).toBe("21,324 strings: dialogue 18,204, skits 3,120 · 1 character renamed");
    expect(whatItChanges({ ...pack, names: 3 })).toBe("3 characters renamed");
  });

  it("gives a date and a time on this computer's clock", () => {
    const moment = new Date(2026, 10, 23, 17, 20);
    // Some versions put a narrow space before PM; any space will do.
    expect(dateAndTime(moment.getTime() / 1000).replace(/\s/g, " ")).toBe("Nov 23, 2026, 5:20 PM");
  });

  it("takes the last part of a path on either system", () => {
    expect(fileName("C:\\games\\tod2.iso")).toBe("tod2.iso");
    expect(fileName("/home/me/games/top.sfc")).toBe("top.sfc");
    expect(fileName("top.sfc")).toBe("top.sfc");
  });
});
