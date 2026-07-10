/*
 * Focused official-v3.3.8 oracle for PrisonLevel.createMobs().
 *
 * This compiles against the pinned game JARs. The synthetic painted level
 * isolates the superclass placement orchestration while using the actual
 * Prison MobSpawner classes, constructors, ShadowCaster, PathFinder,
 * Wandmaker quest generation, and Generator decks.
 */
import com.shatteredpixel.shatteredpixeldungeon.Dungeon;
import com.shatteredpixel.shatteredpixeldungeon.Statistics;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Mob;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Thief;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.npcs.Wandmaker;
import com.shatteredpixel.shatteredpixeldungeon.items.Generator;
import com.shatteredpixel.shatteredpixeldungeon.items.potions.Potion;
import com.shatteredpixel.shatteredpixeldungeon.items.rings.Ring;
import com.shatteredpixel.shatteredpixeldungeon.items.scrolls.Scroll;
import com.shatteredpixel.shatteredpixeldungeon.items.wands.Wand;
import com.shatteredpixel.shatteredpixeldungeon.levels.Level;
import com.shatteredpixel.shatteredpixeldungeon.levels.PrisonLevel;
import com.shatteredpixel.shatteredpixeldungeon.levels.Terrain;
import com.shatteredpixel.shatteredpixeldungeon.levels.painters.Painter;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.Room;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.SpecialRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.StandardRoom;
import com.watabou.utils.Point;
import com.watabou.utils.Random;
import com.watabou.utils.Rect;
import com.watabou.utils.SparseArray;

import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;

public final class PrisonMobPlacementOracle {
    private static final class TaggedStandard extends StandardRoom {
        final String tag;
        final int weight;
        final boolean rejectWestStrip;

        TaggedStandard(String tag, Rect bounds, int weight, boolean rejectWestStrip) {
            this.tag = tag;
            this.weight = weight;
            this.rejectWestStrip = rejectWestStrip;
            set(bounds);
        }

        @Override public void paint(Level level) {}
        @Override public int mobSpawnWeight() { return weight; }

        @Override public boolean canPlaceCharacter(Point point, Level level) {
            return super.canPlaceCharacter(point, level)
                    && (!rejectWestStrip || point.x != left + 1);
        }
    }

    private static final class OracleLevel extends PrisonLevel {
        final TaggedStandard entranceRoom;
        final int fixedEntrance;
        final int fixedExit;

        OracleLevel(boolean large, boolean forceWandmakerFallback) {
            setSize(34, 20);
            for (int i = 0; i < map.length; i++) map[i] = Terrain.WALL;
            mobs = new HashSet<>();
            heaps = new SparseArray<>();
            plants = new SparseArray<>();
            traps = new SparseArray<>();
            blobs = new HashMap<>();

            entranceRoom = new TaggedStandard("E", new Rect(1, 1, 7, 8), 1, false);
            TaggedStandard a = new TaggedStandard("A", new Rect(9, 1, 16, 9), 2, true);
            TaggedStandard b = new TaggedStandard("B", new Rect(18, 1, 25, 9), 1, false);
            TaggedStandard exitRoom = new TaggedStandard("X", new Rect(26, 11, 32, 18), 1, false);
            rooms = new ArrayList<>();
            rooms.add(entranceRoom);
            rooms.add(a);
            rooms.add(b);
            rooms.add(exitRoom);
            roomEntrance = entranceRoom;
            for (Room room : rooms) {
                for (int y = room.top + 1; y < room.bottom; y++) {
                    for (int x = room.left + 1; x < room.right; x++) {
                        map[x + y * width()] = Terrain.EMPTY;
                    }
                }
            }
            fixedEntrance = 3 + 3 * width();
            fixedExit = 29 + 15 * width();
            if (forceWandmakerFallback) {
                for (int y = 3; y <= 6; y++) {
                    for (int x = 3; x <= 5; x++) map[x + y * width()] = Terrain.EMPTY_SP;
                }
            }
            map[fixedEntrance] = Terrain.ENTRANCE;
            map[fixedExit] = Terrain.EXIT;
            feeling = large ? Feeling.LARGE : Feeling.NONE;
            buildFlagMaps();
        }

        @Override protected Painter painter() { return null; }
        @Override protected void createItems() {}
        @Override public int entrance() { return fixedEntrance; }
        @Override public int exit() { return fixedExit; }
        void generateMobs() { super.createMobs(); }
    }

    private static void initGenerator(long dungeonSeed) {
        Dungeon.seed = dungeonSeed;
        Dungeon.depth = 6;
        Dungeon.branch = 0;
        Dungeon.challenges = 0;
        Dungeon.mobsToChampion = 1;
        Statistics.amuletObtained = false;
        Random.resetGenerators();
        Random.pushGenerator(dungeonSeed + 1);
        Scroll.initLabels();
        Potion.initColors();
        Ring.initGems();
        SpecialRoom.initForRun();
        SecretRoom.initForRun();
        Generator.fullReset();
        Random.popGenerator();
    }

    private static void setQuestRoomSpawned(boolean value) throws Exception {
        Field field = Wandmaker.Quest.class.getDeclaredField("questRoomSpawned");
        field.setAccessible(true);
        field.setBoolean(null, value);
    }

    private static String wand(Wand wand) {
        return wand == null ? "none" : wand.getClass().getSimpleName() + ":+" + wand.level()
                + ":" + (wand.cursed ? "cursed" : "clean");
    }

    private static String thiefLoot(Mob mob) throws Exception {
        if (!(mob instanceof Thief)) return "-";
        Field field = Mob.class.getDeclaredField("loot");
        field.setAccessible(true);
        Object loot = field.get(mob);
        return loot instanceof Generator.Category ? ((Generator.Category)loot).name() : String.valueOf(loot);
    }

    private static void run(int depth, long dungeonSeed, long outerSeed,
                            boolean large, boolean wandmaker, boolean fallback) throws Exception {
        initGenerator(dungeonSeed);
        Dungeon.depth = depth;
        Wandmaker.Quest.reset();

        // Room constructors have their own real initializer draws, but this
        // focused fixture starts at the createMobs phase.
        Random.pushGenerator(999);
        OracleLevel level = new OracleLevel(large, fallback);
        Random.popGenerator();
        Dungeon.level = level;
        setQuestRoomSpawned(wandmaker);

        Random.pushGenerator(outerSeed);
        level.generateMobs();
        ArrayList<Mob> mobs = new ArrayList<>(level.mobs);
        mobs.sort(Comparator.comparingInt((Mob mob) -> mob.pos)
                .thenComparing(mob -> mob.getClass().getSimpleName()));
        StringBuilder values = new StringBuilder();
        for (Mob mob : mobs) {
            if (values.length() > 0) values.append(',');
            values.append(mob.getClass().getSimpleName()).append('@').append(mob.pos)
                    .append(':').append(thiefLoot(mob));
        }
        long next = Random.Long();
        Random.popGenerator();
        System.out.printf(
                "depth=%d dungeon=%d outer=%d large=%s quest=%s fallback=%s mobs=%s "
                        + "wand1=%s wand2=%s wandDropped=%d next=%d%n",
                depth, dungeonSeed, outerSeed, large, wandmaker, fallback, values,
                wand(Wandmaker.Quest.wand1), wand(Wandmaker.Quest.wand2),
                Generator.Category.WAND.dropped, next);
    }

    public static void main(String[] args) throws Exception {
        run(6, 0, 123, false, false, false);
        run(8, 0, 9876, true, false, false);
        run(9, 0, 54321, false, true, false);
        run(9, 0, 24680, false, true, true);
    }
}
