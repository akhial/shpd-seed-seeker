/*
 * Shattered Pixel Dungeon Java baseline seed finder
 * Copyright (C) 2026
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */

package com.shatteredpixel.shatteredpixeldungeon;

import com.badlogic.gdx.Preferences;
import com.badlogic.gdx.utils.JsonReader;
import com.badlogic.gdx.utils.JsonValue;
import com.shatteredpixel.shatteredpixeldungeon.items.Generator;
import com.shatteredpixel.shatteredpixeldungeon.levels.Terrain;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import com.shatteredpixel.shatteredpixeldungeon.actors.blobs.SacrificialFire;
import com.shatteredpixel.shatteredpixeldungeon.actors.hero.HeroClass;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.ArmoredStatue;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Mimic;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Mob;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Statue;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.npcs.Blacksmith;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.npcs.Ghost;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.npcs.Imp;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.npcs.Wandmaker;
import com.shatteredpixel.shatteredpixeldungeon.items.Heap;
import com.shatteredpixel.shatteredpixeldungeon.items.Item;
import com.shatteredpixel.shatteredpixeldungeon.items.armor.Armor;
import com.shatteredpixel.shatteredpixeldungeon.items.rings.Ring;
import com.shatteredpixel.shatteredpixeldungeon.items.wands.Wand;
import com.shatteredpixel.shatteredpixeldungeon.items.weapon.Weapon;
import com.shatteredpixel.shatteredpixeldungeon.levels.CityBossLevel;
import com.shatteredpixel.shatteredpixeldungeon.levels.Level;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.ImpShopRoom;
import com.shatteredpixel.shatteredpixeldungeon.utils.DungeonSeed;
import com.watabou.noosa.Game;
import com.watabou.utils.GameSettings;

import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * A seed finder for Shattered Pixel Dungeon v4.0.0, driven headlessly
 * against the unmodified official desktop JAR.
 *
 * <p>It exists so that Seed Seeker's throughput can be compared with what the
 * game's own generator achieves on the JVM at the version Seed Seeker targets.
 * The startup technique is the one {@code tooling/oracle-4.0} documents:
 * the final v4.0.0 source is published, but this baseline runs the shipped JAR
 * without recompiling the game. Only the headless {@code TextureFilm} and
 * {@code ItemSprite} stand-ins precede it on the classpath.
 *
 * <p>Per seed the finder runs the game's own {@code Dungeon.init()} and
 * {@code Dungeon.newLevel()} over floors 1..N, plus the Imp's Vault when the
 * Imp has spawned, and scans everything generated — heaps and their containers,
 * mimics, statues, the sacrificial-fire prize, the Imp's shop cache and the
 * Ghost/Wandmaker/Blacksmith/Imp reward options — for an item of a named class
 * at a named upgrade and optional enchantment/glyph. The game's generator state is global, so one JVM searches
 * one seed at a time; use several processes over disjoint ranges for a
 * multi-core figure.
 */
public final class JarSeedFinder {

	private static final String GAME_VERSION = "4.0.0";
	private static final int GAME_VERSION_CODE = 912;
	/** {@code DeviceCompat.isDebug()} is {@code Game.version.contains("INDEV")}; see the oracle's README. */
	private static final String EFFECTIVE_GAME_VERSION = GAME_VERSION + "-INDEV";

	/** The Imp is guaranteed to have spawned by this depth. */
	private static final int LAST_IMP_DEPTH = 19;

	private JarSeedFinder() {
	}

	public static void main(String[] args) {
		try {
			Options options = Options.parse(args);
			if (options.help) {
				printUsage();
				return;
			}
			run(options);
		} catch (IllegalArgumentException error) {
			System.err.println("java-finder: " + error.getMessage());
			System.err.println("Try --help for usage.");
			System.exit(2);
		} catch (Throwable error) {
			error.printStackTrace(System.err);
			System.exit(1);
		}
	}

	private static void run(Options options) throws Exception {
		Game.version = EFFECTIVE_GAME_VERSION;
		Game.versionCode = GAME_VERSION_CODE;
		GameSettings.set(new MemoryPreferences());
		SPDSettings.intro(false);
		setStaticField(Badges.class, "global", new HashSet<Badges.Badge>());
		setStaticField(Bones.class, "depth", 0);
		setStaticField(Bones.class, "branch", -1);
		setStaticField(Bones.class, "item", null);
		setStaticField(Bones.class, "heroClass", null);
		Dungeon.daily = false;
		Dungeon.dailyReplay = false;
		SPDSettings.challenges(options.challenges);

		if (options.stream) {
			runStream(options);
			return;
		}

		List<String> matches = new ArrayList<String>();
		// The JIT needs a few hundred runs before the generator settles; the
		// warmup seeds are searched exactly like the rest but not timed, which
		// is the friendlier of the two readings for the JVM.
		for (long index = 0; index < options.warmup; index++) {
			search(options.start + index, options);
		}

		long tested = 0;
		long startedAt = System.nanoTime();
		for (long index = 0; index < options.seeds; index++) {
			long seed = options.start + options.warmup + index;
			if (search(seed, options)) matches.add(DungeonSeed.convertToCode(seed));
			tested++;
		}
		double elapsed = (System.nanoTime() - startedAt) / 1_000_000_000.0;

		if (options.printMatches) {
			for (String code : matches) System.out.println(code);
		}
		System.out.printf(Locale.ROOT,
				"BENCH item=%s+%d floors=%d start=%d warmup=%d seeds=%d matches=%d "
						+ "elapsed=%.3f seeds_per_s=%.1f%n",
				options.item, options.upgrade, options.floors, options.start + options.warmup,
				options.warmup, tested, matches.size(), elapsed, tested / elapsed);
	}

	/** JSON-lines benchmark/replay adapter. Input parsing and output are outside
	 * the internal timer; the driver also records end-to-end batch wall time. */
	private static void runStream(Options options) throws Exception {
		for (long i = 0; i < options.warmup; i++) search(9_000_000L + i, options);
		System.out.println("{\"ready\":true}");
		BufferedReader input = new BufferedReader(new InputStreamReader(System.in));
		String line;
		while ((line = input.readLine()) != null) {
			JsonValue request = new JsonReader().parse(line);
			long[] seeds = request.get("seeds").asLongArray();
			JsonValue choices = request.get("trinkets");
			options.fullScan = request.getBoolean("verify", false);
			if (choices != null && choices.size != seeds.length) throw new IllegalArgumentException("one choice per seed required");
			List<String> results = new ArrayList<>();
			long began = System.nanoTime();
			for (int i = 0; i < seeds.length; i++) {
				options.trinket = choices == null || choices.get(i).isNull() ? null : choices.get(i).asString();
				boolean found = search(seeds[i], options);
				if (found || options.fullScan) results.add("{\"seed\":" + seeds[i]
					+ ",\"witnesses\":[" + String.join(",", options.witnesses) + "]}");
			}
			double seconds = (System.nanoTime() - began) / 1e9;
			System.out.println("{\"tested\":" + seeds.length + ",\"seconds\":" + seconds
				+ ",\"matches\":[" + String.join(",", results) + "]}");
		}
	}

	/** Generates one seed's world and reports whether it holds the wanted item. */
	private static boolean search(long seed, Options options) throws Exception {
		SPDSettings.customSeed(DungeonSeed.convertToCode(seed));
		Dungeon.initSeed();
		GamesInProgress.selectedClass = HeroClass.WARRIOR;
		Dungeon.init();
		resetLeftoverQuestState();

		options.witnesses.clear();
		options.foundItems.clear();
		options.independentItems.clear();
		options.source = "Heap";
		options.effectOverride = null;
		boolean brewed = false;
		boolean alchemy = false;
		int impDepth = -1;
		boolean ghost = false;
		boolean wandmaker = false;
		boolean blacksmith = false;
		boolean found = false;
		for (int depth = 1; depth <= (options.fullScan ? 24 : options.floors) && (!found || options.fullScan); depth++) {
			options.depth = depth;
			// Depths 5, 10, 15 and 25 leave no run-persistent state behind — the
			// oracle's boss-skip fixtures pin that — so a search can step over
			// them. Depth 20 is not neutral (it caches the Imp's shop) and is
			// never skipped.
			if (options.skipBossFloors && isSkippableBossDepth(depth)) {
				Dungeon.depth++;
				continue;
			}
			Level level = Dungeon.newLevel();
			found |= matches(level, options);

			if (!ghost && Ghost.Quest.weapon != null && Ghost.Quest.armor != null) {
				ghost = true;
				options.source = "GhostReward";
				options.effectOverride = Ghost.Quest.enchant;
				found |= matches(Ghost.Quest.weapon, options);
				options.effectOverride = Ghost.Quest.glyph;
				found |= matches(Ghost.Quest.armor, options);
				options.effectOverride = null;
			}
			if (!wandmaker && Wandmaker.Quest.wand1 != null && Wandmaker.Quest.wand2 != null) {
				wandmaker = true;
				options.source = "WandmakerReward";
				found |= matches(Wandmaker.Quest.wand1, options) | matches(Wandmaker.Quest.wand2, options);
			}
			if (!blacksmith && Blacksmith.Quest.smithRewards != null) {
				blacksmith = true;
				options.source = "BlacksmithReward";
				found |= matches(Blacksmith.Quest.smithRewards, options);
			}
			// Imp.Quest.rewardOptions is rolled on the Imp's City floor and cleared
			// again by VaultFinalRoom.paint(), so it has to be read here.
			if (impDepth < 0 && !Imp.Quest.rewardOptions.isEmpty()) {
				impDepth = depth;
				options.source = "ImpReward";
				found |= matches(Imp.Quest.rewardOptions, options);
			}

			// Apply only after the same catalyst/alchemy opportunity as the engine.
			if (options.trinket != null && !brewed) {
				for (int tile : level.map) if (tile == Terrain.ALCHEMY) alchemy = true;
				if (alchemy && Dungeon.LimitedDrops.TRINKET_CATA.count > 0) {
					Item trinket = (Item) Class.forName("com.shatteredpixel.shatteredpixeldungeon.items.trinkets." + options.trinket).getDeclaredConstructor().newInstance();
					trinket.level(3);
					Dungeon.hero.belongings.backpack.items.add(trinket);
					brewed = true;
				}
			}
			Dungeon.depth++;
		}

		if ((!found || options.fullScan) && options.vault && impDepth > 0) found |= searchVault(impDepth, options);

		// Read the private offer deck only after all level generation is finished.
		if (options.trinket != null) {
			boolean offered = false;
			for (int i = 0; i < 4; i++) offered |= Generator.random(Generator.Category.TRINKET).getClass().getSimpleName().equals(options.trinket);
			if (!offered) throw new IllegalArgumentException("trinket was not initially offered: " + options.trinket);
		}
		return found;
	}

	/** Boss depths whose generation is run-state neutral; depth 20 is not. */
	private static boolean isSkippableBossDepth(int depth) {
		return depth == 5 || depth == 10 || depth == 15 || depth == 25;
	}

	/**
	 * Restores the run statics that {@code Dungeon.init()} leaves alone because
	 * the game never searches two seeds in one process.
	 *
	 * <p>{@code Imp.Quest.reset()} does not touch {@code rewardOptions},
	 * {@code oldQuest} or {@code alternative}: the reward options are rolled on
	 * the Imp's floor and cleared again by {@code VaultFinalRoom.paint()}, and
	 * the two flags are assigned by {@code Imp.Quest.spawn()}. A run whose vault
	 * is never built therefore hands its reward options to the next seed in the
	 * loop, where they would be read as that seed's own; the values a fresh JVM
	 * would hold are restored here instead.
	 */
	private static void resetLeftoverQuestState() throws Exception {
		Imp.Quest.rewardOptions.clear();
		setStaticField(Imp.Quest.class, "oldQuest", Boolean.FALSE);
		setStaticField(Imp.Quest.class, "alternative", Boolean.FALSE);
	}

	/**
	 * Builds the Imp's Vault (branch 1 of the Imp's floor) and scans it. The
	 * sub-level is seeded independently by {@code Dungeon.seedForDepth(depth, 1)}
	 * and leaves no run-persistent state behind, which is why it can be built
	 * after the main floors rather than between them; {@code tooling/oracle-4.0}
	 * pins that neutrality.
	 */
	private static boolean searchVault(int impDepth, Options options) throws Exception {
		int savedDepth = Dungeon.depth;
		int savedBranch = Dungeon.branch;
		options.depth = impDepth;
		Dungeon.depth = impDepth;
		Dungeon.branch = 1;
		try {
			return matches(Dungeon.newLevel(), options);
		} finally {
			Dungeon.depth = savedDepth;
			Dungeon.branch = savedBranch;
		}
	}

	/** Scans everything a generated floor carries: heaps, mimics, statues, prizes. */
	private static boolean matches(Level level, Options options) throws Exception {
		boolean found = false;
		for (Heap heap : level.heaps.valueList()) {
			options.source = Dungeon.branch == 1 ? "VaultTreasure" : switch (heap.type) {
				case HEAP -> "Heap"; case CHEST -> "Chest"; case LOCKED_CHEST -> "LockedChest";
				case CRYSTAL_CHEST -> "CrystalChest"; case TOMB -> "Tomb";
				case SKELETON -> "Skeleton"; case FOR_SALE -> "Shop";
				default -> throw new IllegalStateException("unknown heap: " + heap.type);
			};
			if (Dungeon.branch == 1 && level instanceof com.shatteredpixel.shatteredpixeldungeon.levels.RegularLevel) {
				Object room = ((com.shatteredpixel.shatteredpixeldungeon.levels.RegularLevel)level).room(heap.pos);
				if (room != null && room.getClass().getSimpleName().equals("VaultFinalRoom")) continue;
			}
			found |= matches(heap.items, options);
			if (found && !options.fullScan) return true;
		}
		for (Mob mob : level.mobs) {
			if (mob instanceof Mimic) {
				options.source = Dungeon.branch == 1 ? "VaultTreasure" : mob instanceof com.shatteredpixel.shatteredpixeldungeon.actors.mobs.GoldenMimic ? "GoldenMimic" : mob instanceof com.shatteredpixel.shatteredpixeldungeon.actors.mobs.CrystalMimic ? "CrystalMimic" : "Mimic";
				found |= matches(((Mimic)mob).items, options);
			} else if (mob instanceof Statue) {
				options.source = Dungeon.branch == 1 ? "VaultTreasure" : mob instanceof ArmoredStatue ? "ArmoredStatue" : "Statue";
				found |= matches(((Statue)mob).weapon(), options);
				if (mob instanceof ArmoredStatue) found |= matches(((ArmoredStatue)mob).armor(), options);
			}
			if (found && !options.fullScan) return true;
		}
		options.source = "SacrificialFire";
		SacrificialFire fire = (SacrificialFire)level.blobs.get(SacrificialFire.class);
		if (fire != null) found |= matches(getField(fire, "prize"), options);
		if (level instanceof CityBossLevel) {
			options.source = "Shop";
			Object shop = getField(level, "impShop");
			if (shop instanceof ImpShopRoom) found |= matches((Collection<?>)getField(shop, "itemsToSpawn"), options);
		}
		return found;
	}

	private static boolean matches(Collection<?> items, Options options) {
		if (items == null) return false;
		boolean found = false;
		for (Object item : items) {
			found |= matches(item, options);
			if (found && !options.fullScan) return true;
		}
		return found;
	}

	/**
	 * The match itself: an item of the wanted class at the wanted upgrade.
	 * {@code trueLevel()} is the upgrade the item really carries — a cursed
	 * item's displayed level is lower — and reading it, unlike {@code name()}
	 * or {@code identify()}, does not mutate the item.
	 */
	private static boolean matches(Object candidate, Options options) {
		if (!(candidate instanceof Item)) return false;
		Item item = (Item) candidate;
		if (!isSearchable(item)) return false;
		String itemClass = item.getClass().getSimpleName();
		if (!options.items.contains(itemClass)) return false;
		if (options.depth > options.floors || (options.upgrade >= 0 && item.trueLevel() != options.upgrade)) return false;
		Object effect = item instanceof Weapon ? ((Weapon)item).enchantment : item instanceof Armor ? ((Armor)item).glyph : null;
		if (options.source.equals("GhostReward")) effect = options.effectOverride;
		if (options.source.equals("BlacksmithReward")) effect = item instanceof Weapon ? Blacksmith.Quest.smithEnchant : item instanceof Armor ? Blacksmith.Quest.smithGlyph : null;
		String effectName = effect == null ? "-" : effect.getClass().getSimpleName();
		if (effectName.equals("AntiMagic")) effectName = "Anti-Magic";
		if (effectName.equals("AntiEntropy")) effectName = "Anti-Entropy";
		if (options.effect != null && itemClass.equals(options.items.get(0))
				&& !options.effects.contains(effectName)) return false;
		String id = item.getClass().getSimpleName().replaceAll("(?<!^)([A-Z])", "_$1").toLowerCase(Locale.ROOT).replace("wand_of_", "wand_").replace("ring_of_", "ring_");
		options.witnesses.add("[" + options.depth + ",\"" + options.source + "\",\"" + id + "\"," + item.trueLevel() + "," + item.cursed + ",\"" + effectName + "\"]");
		options.foundItems.add(itemClass);
		// The benchmark's blade and ring have no shared choice outside the
		// vault. Exactly one item can leave the vault, including Imp rewards.
		if (!options.source.equals("ImpReward") && !options.source.equals("VaultTreasure")) {
			options.independentItems.add(itemClass);
		}
		return options.foundItems.containsAll(options.items)
				&& options.items.stream().filter(required -> !options.independentItems.contains(required)).count() <= 1;
	}

	/** The kinds Seed Seeker searches, so that both tools answer the same question. */
	private static boolean isSearchable(Item item) {
		return item instanceof Weapon || item instanceof Armor || item instanceof Wand
				|| item instanceof Ring;
	}

	private static Object getField(Object owner, String name) throws Exception {
		return findField(owner.getClass(), name).get(owner);
	}

	private static void setStaticField(Class<?> type, String name, Object value) throws Exception {
		findField(type, name).set(null, value);
	}

	private static Field findField(Class<?> type, String name) throws NoSuchFieldException {
		for (Class<?> current = type; current != null; current = current.getSuperclass()) {
			try {
				Field field = current.getDeclaredField(name);
				field.setAccessible(true);
				return field;
			} catch (NoSuchFieldException ignored) {
				// keep walking up
			}
		}
		throw new NoSuchFieldException(type.getName() + "." + name);
	}

	private static void printUsage() {
		System.out.println("Usage: java-finder [--item CLASS] [--upgrade N] [--floors N] "
				+ "[--seeds N] [--start N] [--warmup N] [--challenges N] [--no-vault]"
				+ " [--skip-boss-floors] [--print-matches] [--effect NAME] [--stream]");
		System.out.println("  --item CLASS       Comma-separated required item classes (default: RunicBlade)");
		System.out.println("  --stream           JSON-lines batch search and recipe verification protocol");
		System.out.println("  --effect NAME      Allowed effects for the first item, comma-separated (default: any)");
		System.out.println("  --upgrade N        Required true upgrade, -1 for any (default: 5)");
		System.out.println("  --floors N         Deepest floor to generate (default: 19)");
		System.out.println("  --seeds N          Timed seeds (default: 2000)");
		System.out.println("  --start N          First numeric seed (default: 0)");
		System.out.println("  --warmup N         Untimed seeds searched first (default: 200)");
		System.out.println("  --challenges N     Challenge bit mask (default: 0)");
		System.out.println("  --no-vault         Do not build the Imp's Vault");
		System.out.println("  --skip-boss-floors Step over the state-neutral boss depths 5, 10, 15, 25");
		System.out.println("  --print-matches    Print each matching seed code before the BENCH line");
	}

	private static final class Options {
		String item = "RunicBlade";
		String effect;
		List<String> items;
		List<String> effects;
		java.util.Set<String> foundItems = new java.util.HashSet<>();
		java.util.Set<String> independentItems = new java.util.HashSet<>();
		boolean stream;
		boolean fullScan;
		String trinket;
		String source;
		Object effectOverride;
		int depth;
		List<String> witnesses = new ArrayList<>();
		int upgrade = 5;
		int floors = LAST_IMP_DEPTH;
		long seeds = 2000;
		long start;
		long warmup = 200;
		int challenges;
		boolean vault = true;
		boolean skipBossFloors;
		boolean printMatches;
		boolean help;

		static Options parse(String[] args) {
			Options result = new Options();
			for (int i = 0; i < args.length; i++) {
				String arg = args[i];
				if ("--help".equals(arg) || "-h".equals(arg)) {
					result.help = true;
				} else if ("--item".equals(arg)) {
					result.item = requireValue(args, ++i, arg);
				} else if ("--effect".equals(arg)) {
					result.effect = requireValue(args, ++i, arg);
				} else if ("--stream".equals(arg)) {
					result.stream = true;
				} else if ("--upgrade".equals(arg)) {
					result.upgrade = Integer.parseInt(requireValue(args, ++i, arg));
				} else if ("--floors".equals(arg)) {
					result.floors = Integer.parseInt(requireValue(args, ++i, arg));
				} else if ("--seeds".equals(arg)) {
					result.seeds = Long.parseLong(requireValue(args, ++i, arg));
				} else if ("--start".equals(arg)) {
					result.start = Long.parseLong(requireValue(args, ++i, arg));
				} else if ("--warmup".equals(arg)) {
					result.warmup = Long.parseLong(requireValue(args, ++i, arg));
				} else if ("--challenges".equals(arg)) {
					result.challenges = Integer.parseInt(requireValue(args, ++i, arg));
				} else if ("--no-vault".equals(arg)) {
					result.vault = false;
				} else if ("--skip-boss-floors".equals(arg)) {
					result.skipBossFloors = true;
				} else if ("--print-matches".equals(arg)) {
					result.printMatches = true;
				} else {
					throw new IllegalArgumentException("unknown option '" + arg + "'");
				}
			}
			result.items = List.of(result.item.split(","));
			result.effects = result.effect == null ? List.of() : List.of(result.effect.split(","));
			if (result.items.size() > 1 && (!result.items.equals(List.of("RunicBlade", "RingOfMight"))
					|| result.upgrade != 2 || result.floors != 19)) {
				throw new IllegalArgumentException("compound mode supports only +2 RunicBlade and +2 RingOfMight through floor 19");
			}
			if (result.floors < 1 || result.floors > 26) {
				throw new IllegalArgumentException("--floors must be between 1 and 26");
			}
			if (result.seeds < 1) throw new IllegalArgumentException("--seeds must be positive");
			if (result.warmup < 0) throw new IllegalArgumentException("--warmup cannot be negative");
			return result;
		}

		private static String requireValue(String[] args, int index, String option) {
			if (index >= args.length) {
				throw new IllegalArgumentException(option + " requires a value");
			}
			return args[index];
		}
	}

	private static final class MemoryPreferences implements Preferences {
		private final Map<String, Object> values = new HashMap<String, Object>();

		@Override public Preferences putBoolean(String key, boolean val) { values.put(key, val); return this; }
		@Override public Preferences putInteger(String key, int val) { values.put(key, val); return this; }
		@Override public Preferences putLong(String key, long val) { values.put(key, val); return this; }
		@Override public Preferences putFloat(String key, float val) { values.put(key, val); return this; }
		@Override public Preferences putString(String key, String val) { values.put(key, val); return this; }
		@Override public Preferences put(Map<String, ?> vals) { values.putAll(vals); return this; }
		@Override public boolean getBoolean(String key) { return getBoolean(key, false); }
		@Override public int getInteger(String key) { return getInteger(key, 0); }
		@Override public long getLong(String key) { return getLong(key, 0L); }
		@Override public float getFloat(String key) { return getFloat(key, 0f); }
		@Override public String getString(String key) { return getString(key, ""); }
		@Override public boolean getBoolean(String key, boolean defValue) { Object v = values.get(key); return v instanceof Boolean ? (Boolean) v : defValue; }
		@Override public int getInteger(String key, int defValue) { Object v = values.get(key); return v instanceof Number ? ((Number) v).intValue() : defValue; }
		@Override public long getLong(String key, long defValue) { Object v = values.get(key); return v instanceof Number ? ((Number) v).longValue() : defValue; }
		@Override public float getFloat(String key, float defValue) { Object v = values.get(key); return v instanceof Number ? ((Number) v).floatValue() : defValue; }
		@Override public String getString(String key, String defValue) { Object v = values.get(key); return v instanceof String ? (String) v : defValue; }
		@Override public Map<String, ?> get() { return new HashMap<String, Object>(values); }
		@Override public boolean contains(String key) { return values.containsKey(key); }
		@Override public void clear() { values.clear(); }
		@Override public void remove(String key) { values.remove(key); }
		@Override public void flush() { }
	}
}
