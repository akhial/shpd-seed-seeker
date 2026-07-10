/*
 * Focused official-v3.3.8 oracle for inherited CavesLevel.createMobs().
 *
 * This compiles against the pinned game JARs. The synthetic painted level
 * isolates RegularLevel's placement orchestration while using the actual
 * Caves MobSpawner classes, constructors, ShadowCaster, and PathFinder. An
 * optional pre-painted Blacksmith proves that quest rewards/NPC construction
 * do not belong to this later phase while its occupancy does.
 */
import com.shatteredpixel.shatteredpixeldungeon.Dungeon;
import com.shatteredpixel.shatteredpixeldungeon.Statistics;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.DM200;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Mob;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.npcs.Blacksmith;
import com.shatteredpixel.shatteredpixeldungeon.items.Generator;
import com.shatteredpixel.shatteredpixeldungeon.items.potions.Potion;
import com.shatteredpixel.shatteredpixeldungeon.items.rings.Ring;
import com.shatteredpixel.shatteredpixeldungeon.items.scrolls.Scroll;
import com.shatteredpixel.shatteredpixeldungeon.levels.CavesLevel;
import com.shatteredpixel.shatteredpixeldungeon.levels.Level;
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

public final class CavesMobPlacementOracle {
    private static final class TaggedStandard extends StandardRoom {
        final int weight;
        final boolean rejectWestStrip;

        TaggedStandard(Rect bounds, int weight, boolean rejectWestStrip) {
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

    private static final class OracleLevel extends CavesLevel {
        final TaggedStandard entranceRoom;
        final int fixedEntrance;
        final int fixedExit;
        final int blacksmithCell;

        OracleLevel(boolean large, boolean blacksmith) {
            setSize(34, 20);
            for (int i = 0; i < map.length; i++) map[i] = Terrain.WALL;
            mobs = new HashSet<>();
            heaps = new SparseArray<>();
            plants = new SparseArray<>();
            traps = new SparseArray<>();
            blobs = new HashMap<>();

            entranceRoom = new TaggedStandard(new Rect(1, 1, 7, 8), 1, false);
            TaggedStandard a = new TaggedStandard(new Rect(9, 1, 16, 9), 2, true);
            TaggedStandard b = new TaggedStandard(new Rect(18, 1, 25, 9), 1, false);
            TaggedStandard exitRoom = new TaggedStandard(new Rect(26, 11, 32, 18), 1, false);
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
            map[fixedEntrance] = Terrain.ENTRANCE;
            map[fixedExit] = Terrain.EXIT;
            feeling = large ? Feeling.LARGE : Feeling.NONE;

            blacksmithCell = 12 + 5 * width();
            if (blacksmith) {
                map[blacksmithCell] = Terrain.HIGH_GRASS;
                Blacksmith npc = new Blacksmith();
                npc.pos = blacksmithCell;
                mobs.add(npc);
            }
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
        Dungeon.depth = 11;
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

    private static String dmLoot(Mob mob) throws Exception {
        if (!(mob instanceof DM200)) return "-";
        Field field = Mob.class.getDeclaredField("loot");
        field.setAccessible(true);
        Object loot = field.get(mob);
        return loot instanceof Generator.Category ? ((Generator.Category)loot).name() : String.valueOf(loot);
    }

    private static void run(int depth, long dungeonSeed, long outerSeed,
                            boolean large, boolean blacksmith) throws Exception {
        initGenerator(dungeonSeed);
        Dungeon.depth = depth;
        Blacksmith.Quest.reset();

        Random.pushGenerator(999);
        OracleLevel level = new OracleLevel(large, blacksmith);
        Random.popGenerator();
        Dungeon.level = level;

        Random.pushGenerator(outerSeed);
        level.generateMobs();
        ArrayList<Mob> mobs = new ArrayList<>(level.mobs);
        mobs.sort(Comparator.comparingInt((Mob mob) -> mob.pos)
                .thenComparing(mob -> mob.getClass().getSimpleName()));
        StringBuilder values = new StringBuilder();
        for (Mob mob : mobs) {
            if (values.length() > 0) values.append(',');
            values.append(mob.getClass().getSimpleName()).append('@').append(mob.pos)
                    .append(':').append(dmLoot(mob));
        }
        long next = Random.Long();
        Random.popGenerator();
        System.out.printf(
                "depth=%d dungeon=%d outer=%d large=%s blacksmith=%s mobs=%s "
                        + "blacksmithTerrain=%d next=%d%n",
                depth, dungeonSeed, outerSeed, large, blacksmith, values,
                level.map[level.blacksmithCell], next);
    }

    public static void main(String[] args) throws Exception {
        run(11, 0, 123, false, false);
        run(13, 0, 9876, true, false);
        run(14, 0, 54321, false, true);
    }
}
