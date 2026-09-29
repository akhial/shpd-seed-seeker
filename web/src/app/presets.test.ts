import { describe, expect, it } from "vite-plus/test";
import { builtInPresets } from "./store";
import { fromQueryJson } from "../features/query/query";
import { validateQuery } from "../features/query/validation";

/** Each preset as the shared query-document format writes it. */
const documents: [string, string][] = [
  [
    "DISINTEGRATE",
    `{"auto_apply_trinket":true,"max_depth":19,"requirements":[
      {"item":"wand_disintegration","kind":"wand","upgrade":{"at_least":3}},
      {"item":"wand_disintegration","kind":"wand"},
      {"item":"wand_disintegration","kind":"wand"},
      {"item":"eye_of_newt","kind":"trinket","trinket_transmutations":1},
      {"item":"ring_energy","kind":"ring","upgrade":{"at_least":2}}]}`,
  ],
  [
    "Guerilla Assassin",
    `{"auto_apply_trinket":true,"requirements":[
      {"effect":"Blooming","item":"assassins_blade","kind":"weapon","max_depth":7,"upgrade":3},
      {"effect":"Camouflage","kind":"armor"},
      {"item":"ring_arcana","kind":"ring","upgrade":{"at_least":2}}]}`,
  ],
  [
    "Ring of Wealth",
    `{
      "floor_requirements":[{"any_rooms":["garden","secret_garden"],"depth":17,"feeling":"dark"}],
      "requirements":[
      {"item":"ring_wealth","kind":"ring","upgrade":4},
      {"item":"dried_rose","kind":"artifact","max_depth":9},
      {"kind":"armor","max_depth":4,"tier":{"at_most":4},"upgrade":3},
      {"kind":"weapon","max_depth":9,"tier":{"at_most":4},"upgrade":3},
      {"item":"dimensional_sundial","kind":"trinket","trinket_transmutations":1}]}`,
  ],
  [
    "Necromancer",
    `{"auto_apply_trinket":true,"max_depth":14,"wandmaker_quest":"corpse_dust","requirements":[
      {"item":"wand_corruption","kind":"wand","upgrade":3},
      {"kind":"weapon","tier":{"exact":5},"upgrade":3},
      {"item":"plate_armor","kind":"armor","upgrade":3}]}`,
  ],
  [
    "Blood Berserker",
    `{"auto_apply_trinket":true,"requirements":[
      {"effect":"Vampiric","kind":"weapon","tier":{"exact":5},"upgrade":3},
      {"effect":"Thorns","item":"plate_armor","kind":"armor","upgrade":3},
      {"item":"ring_arcana","kind":"ring","upgrade":4},
      {"item":"chalice_of_blood","kind":"artifact"}]}`,
  ],
];

describe("built-in presets", () => {
  it("ships a query the editor accepts", () => {
    for (const preset of builtInPresets) {
      expect(validateQuery(preset.query).errors, preset.name).toEqual([]);
    }
  });

  it("is the query each preset was taken from", () => {
    expect(builtInPresets.map((preset) => preset.name)).toEqual(documents.map(([name]) => name));
    for (const [index, [name, document]] of documents.entries()) {
      expect(builtInPresets[index].query, name).toEqual(fromQueryJson(document));
    }
  });
});
