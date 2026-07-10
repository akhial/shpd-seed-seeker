/*
 * Focused official-v3.3.8 oracle for inherited HallsLevel.createMobs().
 *
 * The synthetic painted level uses the actual Halls MobSpawner classes,
 * constructors, ShadowCaster, PathFinder, weighted room traversal, and retry
 * loops. An optional pre-painted DemonSpawner proves the phase boundary.
 */
import com.shatteredpixel.shatteredpixeldungeon.Dungeon;
import com.shatteredpixel.shatteredpixeldungeon.Statistics;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.DemonSpawner;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Mob;
import com.shatteredpixel.shatteredpixeldungeon.items.Generator;
import com.shatteredpixel.shatteredpixeldungeon.items.potions.Potion;
import com.shatteredpixel.shatteredpixeldungeon.items.rings.Ring;
import com.shatteredpixel.shatteredpixeldungeon.items.scrolls.Scroll;
import com.shatteredpixel.shatteredpixeldungeon.levels.HallsLevel;
import com.shatteredpixel.shatteredpixeldungeon.levels.Level;
import com.shatteredpixel.shatteredpixeldungeon.levels.Terrain;
import com.shatteredpixel.shatteredpixeldungeon.levels.painters.Painter;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.Room;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.SpecialRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.StandardRoom;
import com.watabou.noosa.Game;
import com.watabou.utils.Point;
import com.watabou.utils.Random;
import com.watabou.utils.Rect;
import com.watabou.utils.SparseArray;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;

public final class HallsMobPlacementOracle {
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

    private static final class OracleLevel extends HallsLevel {
        final TaggedStandard entranceRoom;
        final int fixedEntrance;
        final int fixedExit;
        final int spawnerCell;

        OracleLevel(boolean large, boolean spawner) {
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

            spawnerCell = 12 + 5 * width();
            if (spawner) {
                map[spawnerCell] = Terrain.HIGH_GRASS;
                DemonSpawner actor = new DemonSpawner();
                actor.pos = spawnerCell;
                actor.spawnRecorded = true;
                mobs.add(actor);
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
        Game.version = "3.3.8";
        Dungeon.seed = dungeonSeed;
        Dungeon.depth = 21;
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

    private static void run(int depth, long dungeonSeed, long outerSeed,
                            boolean large, boolean spawner) {
        initGenerator(dungeonSeed);
        Dungeon.depth = depth;

        Random.pushGenerator(999);
        OracleLevel level = new OracleLevel(large, spawner);
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
            values.append(mob.getClass().getSimpleName()).append('@').append(mob.pos);
        }
        long next = Random.Long();
        Random.popGenerator();
        System.out.printf(
                "depth=%d dungeon=%d outer=%d large=%s spawner=%s mobs=%s "
                        + "spawnerTerrain=%d next=%d%n",
                depth, dungeonSeed, outerSeed, large, spawner, values,
                level.map[level.spawnerCell], next);
    }

    public static void main(String[] args) {
        run(21, 0, 123, false, false);
        run(23, 0, 9876, true, false);
        run(24, 0, 54321, false, true);
    }
}
