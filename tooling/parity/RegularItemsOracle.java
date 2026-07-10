/* Isolated RegularLevel.createItems fixtures through actual v3.3.8 classes. */
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.backends.lwjgl3.Lwjgl3Files;
import com.badlogic.gdx.utils.GdxNativesLoader;
import com.shatteredpixel.shatteredpixeldungeon.Dungeon;
import com.shatteredpixel.shatteredpixeldungeon.actors.hero.Hero;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Mimic;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Mob;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.npcs.Blacksmith;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.npcs.Ghost;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.npcs.Imp;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.npcs.Wandmaker;
import com.shatteredpixel.shatteredpixeldungeon.items.Generator;
import com.shatteredpixel.shatteredpixeldungeon.items.Heap;
import com.shatteredpixel.shatteredpixeldungeon.items.Item;
import com.shatteredpixel.shatteredpixeldungeon.items.Stylus;
import com.shatteredpixel.shatteredpixeldungeon.items.armor.Armor;
import com.shatteredpixel.shatteredpixeldungeon.items.potions.Potion;
import com.shatteredpixel.shatteredpixeldungeon.items.potions.PotionOfStrength;
import com.shatteredpixel.shatteredpixeldungeon.items.rings.Ring;
import com.shatteredpixel.shatteredpixeldungeon.items.scrolls.Scroll;
import com.shatteredpixel.shatteredpixeldungeon.items.scrolls.ScrollOfUpgrade;
import com.shatteredpixel.shatteredpixeldungeon.items.trinkets.TrinketCatalyst;
import com.shatteredpixel.shatteredpixeldungeon.items.weapon.Weapon;
import com.shatteredpixel.shatteredpixeldungeon.journal.Document;
import com.shatteredpixel.shatteredpixeldungeon.levels.Level;
import com.shatteredpixel.shatteredpixeldungeon.levels.RegularLevel;
import com.shatteredpixel.shatteredpixeldungeon.levels.Terrain;
import com.shatteredpixel.shatteredpixeldungeon.levels.painters.Painter;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.SpecialRoom;
import com.watabou.utils.Random;
import com.watabou.utils.SparseArray;

import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.LinkedHashMap;

public final class RegularItemsOracle {
    private static final class OracleLevel extends RegularLevel {
        private int nextCell = 9;

        OracleLevel(boolean large) {
            setSize(256, 1);
            for (int i = 0; i < map.length; i++) map[i] = Terrain.EMPTY;
            mobs = new HashSet<>();
            heaps = new SparseArray<>();
            feeling = large ? Feeling.LARGE : Feeling.NONE;
        }

        @Override protected Painter painter() { return null; }
        @Override protected void createMobs() {}
        @Override protected int randomDropCell() { return ++nextCell; }
        @Override protected int randomDropCell(Class roomType) { return ++nextCell; }

        @Override public Heap drop(Item item, int cell) {
            Heap heap = heaps.get(cell);
            if (heap == null) {
                heap = new Heap();
                heap.pos = cell;
                heaps.put(cell, heap);
            }
            heap.items.addFirst(item);
            return heap;
        }

        void queue(Item item) { addItemToSpawn(item); }
        void runItems() { createItems(); }
    }

    @SuppressWarnings("unchecked")
    private static void markAllDocumentsFound() throws Exception {
        Field statesField = Document.class.getDeclaredField("pagesStates");
        statesField.setAccessible(true);
        for (Document document : Document.values()) {
            LinkedHashMap<String, Integer> states =
                    (LinkedHashMap<String, Integer>)statesField.get(document);
            for (String page : states.keySet()) states.put(page, Document.READ);
        }
    }

    private static void initRun(long dungeonSeed) throws Exception {
        GdxNativesLoader.load();
        Gdx.files = new Lwjgl3Files();
        Dungeon.seed = dungeonSeed;
        Dungeon.challenges = 0;
        Dungeon.branch = 0;
        Dungeon.daily = true; // canonical no-bones profile; Bones.get returns immediately
        Dungeon.customSeedText = "oracle";

        Random.resetGenerators();
        Random.pushGenerator(dungeonSeed + 1);
        Scroll.initLabels();
        Potion.initColors();
        Ring.initGems();
        SpecialRoom.initForRun();
        SecretRoom.initForRun();
        Generator.fullReset();
        Random.resetGenerators();

        Dungeon.LimitedDrops.reset();
        Dungeon.hero = new Hero();
        Ghost.Quest.reset();
        Wandmaker.Quest.reset();
        Blacksmith.Quest.reset();
        Imp.Quest.reset();
        markAllDocumentsFound();
    }

    private static String describe(Item item) {
        String effect = "none";
        if (item instanceof Weapon && ((Weapon)item).enchantment != null) {
            effect = ((Weapon)item).enchantment.getClass().getSimpleName();
        } else if (item instanceof Armor && ((Armor)item).glyph != null) {
            effect = ((Armor)item).glyph.getClass().getSimpleName();
        }
        return String.format("%s:q%d:l%d:c%s:e%s",
                item.getClass().getSimpleName(), item.quantity(), item.level(), item.cursed, effect);
    }

    private static void run(int depth, long outerSeed, boolean large, boolean queued)
            throws Exception {
        initRun(0);
        Dungeon.depth = depth;
        OracleLevel level = new OracleLevel(large);
        Dungeon.level = level;
        if (queued) {
            level.queue(new PotionOfStrength());
            level.queue(new ScrollOfUpgrade());
            level.queue(new Stylus());
            level.queue(new TrinketCatalyst());
        }

        Random.pushGenerator(outerSeed);
        level.runItems();
        long next = Random.Long();
        Random.popGenerator();

        ArrayList<Integer> cells = new ArrayList<>();
        for (Heap heap : level.heaps.valueList()) cells.add(heap.pos);
        for (Mob mob : level.mobs) if (mob instanceof Mimic) cells.add(mob.pos);
        cells.sort(Comparator.naturalOrder());

        System.out.printf("depth=%d seed=%d large=%s queued=%s next=%d groups=%d%n",
                depth, outerSeed, large, queued, next, cells.size());
        for (int cell : cells) {
            Heap heap = level.heaps.get(cell);
            if (heap != null) {
                StringBuilder items = new StringBuilder();
                for (Item item : heap.items) {
                    if (items.length() > 0) items.append(',');
                    items.append(describe(item));
                }
                System.out.printf("%03d heap=%s %s%n", cell, heap.type, items);
            } else {
                Mimic mimic = null;
                for (Mob mob : level.mobs) if (mob.pos == cell) mimic = (Mimic)mob;
                StringBuilder items = new StringBuilder();
                for (Item item : mimic.items) {
                    if (items.length() > 0) items.append(',');
                    items.append(describe(item));
                }
                System.out.printf("%03d mimic=%s %s%n",
                        cell, mimic.getClass().getSimpleName(), items);
            }
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length >= 2) {
            int depth = Integer.parseInt(args[0]);
            long seed = Long.parseLong(args[1]);
            boolean large = args.length >= 3 && Boolean.parseBoolean(args[2]);
            boolean queued = args.length >= 4 && Boolean.parseBoolean(args[3]);
            run(depth, seed, large, queued);
            return;
        }
        run(1, 1, false, false);
        run(6, 6, false, true);
        run(11, 11, true, false);
        run(16, 16, false, false);
    }
}
