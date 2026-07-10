import com.shatteredpixel.shatteredpixeldungeon.Dungeon;
import com.shatteredpixel.shatteredpixeldungeon.items.Generator;
import com.shatteredpixel.shatteredpixeldungeon.items.Gold;
import com.shatteredpixel.shatteredpixeldungeon.items.Heap;
import com.shatteredpixel.shatteredpixeldungeon.items.Item;
import com.shatteredpixel.shatteredpixeldungeon.levels.CavesLevel;
import com.shatteredpixel.shatteredpixeldungeon.levels.Level;
import com.shatteredpixel.shatteredpixeldungeon.levels.Terrain;
import com.shatteredpixel.shatteredpixeldungeon.levels.painters.CavesPainter;
import com.shatteredpixel.shatteredpixeldungeon.levels.painters.Painter;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.Room;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.connection.TunnelRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.StandardRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.Trap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.BurningTrap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.ConfusionTrap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.CorrosionTrap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.FrostTrap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.GatewayTrap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.GeyserTrap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.GrippingTrap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.GuardianTrap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.PitfallTrap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.PoisonDartTrap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.RockfallTrap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.StormTrap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.SummoningTrap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.WarpingTrap;
import com.shatteredpixel.shatteredpixeldungeon.plants.Plant;
import com.watabou.utils.Random;
import com.watabou.utils.SparseArray;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.HashSet;

/** Isolated official-v3.3.8 map/RNG oracle for the Rust Caves room module. */
public final class CavesRoomPaintOracle {

    private static final long PAINT_SEED = 0x0123456789abcdefL;
    private static final int DEPTH = 13;

    public static final class FixtureLevel extends CavesLevel {
        @Override
        public Item findPrizeItem() {
            return new Gold(41);
        }

        @Override
        public Heap drop(Item item, int cell) {
            if (item == null) return new Heap();
            Heap heap = heaps.get(cell);
            if (heap == null) {
                heap = new Heap();
                heap.pos = cell;
                heaps.put(cell, heap);
            }
            heap.drop(item);
            return heap;
        }

        @Override
        public Plant plant(Plant.Seed seed, int cell) {
            Plant plant = seed.couch(cell, this);
            plants.put(cell, plant);
            return plant;
        }

        @Override
        public Trap setTrap(Trap trap, int cell) {
            trap.set(cell);
            traps.put(cell, trap);
            return trap;
        }
    }

    public static final class ExposedCavesPainter extends CavesPainter {
        public void decorateFixture(Level level, ArrayList<Room> rooms) {
            decorate(level, rooms);
        }
    }

    private static Room instantiate(String className, StandardRoom.SizeCategory category)
            throws ReflectiveOperationException {
        Random.pushGenerator(17);
        Room room = (Room) Class.forName(className).getDeclaredConstructor().newInstance();
        Random.popGenerator();
        if (room instanceof StandardRoom) {
            ((StandardRoom) room).sizeCat = category;
        }
        return room;
    }

    private static FixtureLevel level() {
        FixtureLevel level = new FixtureLevel();
        level.mobs = new HashSet<>();
        level.heaps = new SparseArray<>();
        level.blobs = new HashMap<>();
        level.plants = new SparseArray<>();
        level.traps = new SparseArray<>();
        level.transitions = new ArrayList<>();
        level.customTiles = new ArrayList<>();
        level.customWalls = new ArrayList<>();
        level.setSize(24, 24);
        return level;
    }

    private static void attach(Room room, int size) {
        int left = 4, top = 4, right = left + size - 1, bottom = top + size - 1;
        room.set(left, top, right, bottom);

        Room first = new TunnelRoom();
        first.set(left - 2, top, left, bottom);
        Room second = new TunnelRoom();
        second.set(right, top, right + 2, bottom);

        Room.Door firstDoor = new Room.Door(left, top + 2);
        Room.Door secondDoor = new Room.Door(right, bottom - 2);
        room.connected.put(first, firstDoor);
        room.connected.put(second, secondDoor);
        first.connected.put(room, firstDoor);
        second.connected.put(room, secondDoor);
        room.neigbours.add(first);
        room.neigbours.add(second);
        first.neigbours.add(room);
        second.neigbours.add(room);
    }

    private static void resetGenerator() {
        Random.pushGenerator(0x7766554433221100L);
        Generator.fullReset();
        Random.popGenerator();
    }

    private static void run(String label, String className,
                            StandardRoom.SizeCategory category, int size)
            throws ReflectiveOperationException {
        resetGenerator();
        FixtureLevel level = level();
        Dungeon.depth = DEPTH;
        Dungeon.seed = 0;
        Dungeon.level = level;
        Room room = instantiate(className, category);
        attach(room, size);

        Random.pushGenerator(PAINT_SEED);
        room.paint(level);
        int next = Random.Int();
        Random.popGenerator();

        StringBuilder doors = new StringBuilder();
        for (Room.Door door : room.connected.values()) {
            if (doors.length() > 0) doors.append(',');
            doors.append(door.type.name());
        }
        System.out.printf(
                "%s|%d|%d|%s|%d|%d|%d|%d|%d%n",
                label,
                Arrays.hashCode(level.map),
                next,
                doors,
                level.heaps.valueList().size(),
                level.mobs.size(),
                level.plants.valueList().size(),
                level.traps.valueList().size(),
                level.transitions.size());
    }

    private static void connect(Room first, Room second) {
        first.connected.put(second, null);
        second.connected.put(first, null);
        first.neigbours.add(second);
        second.neigbours.add(first);
    }

    private static void runAssembled() throws Exception {
        resetGenerator();
        FixtureLevel level = level();
        level.map = new int[0];
        Dungeon.depth = DEPTH;
        Dungeon.seed = 0;
        Dungeon.level = level;

        Room entrance = instantiate(
                "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.entrance.CaveEntranceRoom",
                StandardRoom.SizeCategory.NORMAL);
        Room middle = instantiate(
                "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.CavesFissureRoom",
                StandardRoom.SizeCategory.NORMAL);
        Room exit = instantiate(
                "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.exit.RegionDecoBridgeExitRoom",
                StandardRoom.SizeCategory.NORMAL);
        entrance.set(0, 0, 8, 8);
        middle.set(8, 0, 16, 8);
        exit.set(16, 0, 24, 8);
        connect(entrance, middle);
        connect(middle, exit);
        ArrayList<Room> rooms = new ArrayList<>(Arrays.asList(entrance, middle, exit));

        Painter painter = new CavesPainter()
                .setWater(0.30f, 6)
                .setGrass(0.15f, 3)
                .setTraps(3, new Class<?>[]{
                                BurningTrap.class, PoisonDartTrap.class, FrostTrap.class,
                                StormTrap.class, CorrosionTrap.class, GrippingTrap.class,
                                RockfallTrap.class, GuardianTrap.class, ConfusionTrap.class,
                                SummoningTrap.class, WarpingTrap.class, PitfallTrap.class,
                                GatewayTrap.class, GeyserTrap.class},
                        new float[]{4, 4, 4, 4, 4, 2, 2, 2, 1, 1, 1, 1, 1, 1});

        Random.pushGenerator(PAINT_SEED);
        boolean painted = painter.paint(level, rooms);
        int next = Random.Int();
        Random.popGenerator();
        String order = rooms.stream()
                .map(room -> room.getClass().getSimpleName())
                .reduce((left, right) -> left + "," + right)
                .orElse("");
        System.out.printf("Assembled|%s|%d|%d|%d|%d|%d|%s%n",
                painted,
                Arrays.hashCode(level.map),
                next,
                level.transitions.size(),
                level.traps.valueList().size(),
                level.heaps.valueList().size(),
                order);
    }

    private static void runDecorationMerge() throws Exception {
        FixtureLevel level = level();
        level.setSize(20, 12);
        Dungeon.depth = DEPTH;
        Dungeon.seed = 0;
        Dungeon.level = level;

        Room first = instantiate(
                "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.CaveRoom",
                StandardRoom.SizeCategory.NORMAL);
        Room second = instantiate(
                "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.CirclePitRoom",
                StandardRoom.SizeCategory.NORMAL);
        first.set(1, 1, 9, 9);
        second.set(9, 1, 17, 9);
        first.neigbours.add(second);
        second.neigbours.add(first);
        Painter.fill(level, first, Terrain.WALL);
        Painter.fill(level, first, 1, Terrain.EMPTY);
        Painter.fill(level, second, Terrain.WALL);
        Painter.fill(level, second, 1, Terrain.EMPTY);
        ArrayList<Room> rooms = new ArrayList<>(Arrays.asList(first, second));

        Random.pushGenerator(PAINT_SEED);
        new ExposedCavesPainter().decorateFixture(level, rooms);
        int next = Random.Int();
        Random.popGenerator();
        System.out.printf("DecorationMerge|%d|%d|%d%n",
                Arrays.hashCode(level.map), next, level.traps.valueList().size());
    }

    public static void main(String[] args) throws Exception {
        String standard = "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.";
        run("Cave", standard + "CaveRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("CaveLarge", standard + "CaveRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("CaveGiant", standard + "CaveRoom", StandardRoom.SizeCategory.GIANT, 17);
        run("RegionDecoBridge", standard + "RegionDecoBridgeRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("RegionDecoBridgeLarge", standard + "RegionDecoBridgeRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("CavesFissure", standard + "CavesFissureRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("CavesFissureLarge", standard + "CavesFissureRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("CavesFissureGiant", standard + "CavesFissureRoom", StandardRoom.SizeCategory.GIANT, 17);
        run("CirclePitNormal", standard + "CirclePitRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("CirclePit", standard + "CirclePitRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("CirclePitGiant", standard + "CirclePitRoom", StandardRoom.SizeCategory.GIANT, 17);
        run("CircleWall", standard + "CircleWallRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("CircleWallGiant", standard + "CircleWallRoom", StandardRoom.SizeCategory.GIANT, 17);
        run("Plants", standard + "PlantsRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("Aquarium", standard + "AquariumRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("Platform", standard + "PlatformRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("Burned", standard + "BurnedRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("Fissure", standard + "FissureRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("GrassyGrave", standard + "GrassyGraveRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("Striped", standard + "StripedRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("Study", standard + "StudyRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("SuspiciousChest", standard + "SuspiciousChestRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("Minefield", standard + "MinefieldRoom", StandardRoom.SizeCategory.NORMAL, 9);

        String entrance = standard + "entrance.";
        String exit = standard + "exit.";
        run("CaveEntrance", entrance + "CaveEntranceRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("CaveExit", exit + "CaveExitRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("RegionBridgeEntrance", entrance + "RegionDecoBridgeEntranceRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("RegionBridgeExit", exit + "RegionDecoBridgeExitRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("CavesFissureEntrance", entrance + "CavesFissureEntranceRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("CavesFissureExit", exit + "CavesFissureExitRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("CircleWallEntrance", exit + "CircleWallEntranceRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("CircleWallExit", exit + "CircleWallExitRoom", StandardRoom.SizeCategory.LARGE, 13);

        String connection = "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.connection.";
        run("Tunnel", connection + "TunnelRoom", null, 9);
        run("Walkway", connection + "WalkwayRoom", null, 9);
        run("RingTunnel", connection + "RingTunnelRoom", null, 9);
        run("RingBridge", connection + "RingBridgeRoom", null, 9);
        runDecorationMerge();
        runAssembled();
    }
}
