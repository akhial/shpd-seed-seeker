import com.shatteredpixel.shatteredpixeldungeon.Dungeon;
import com.shatteredpixel.shatteredpixeldungeon.items.Generator;
import com.shatteredpixel.shatteredpixeldungeon.items.Gold;
import com.shatteredpixel.shatteredpixeldungeon.items.Heap;
import com.shatteredpixel.shatteredpixeldungeon.items.Item;
import com.shatteredpixel.shatteredpixeldungeon.levels.CityLevel;
import com.shatteredpixel.shatteredpixeldungeon.levels.Level;
import com.shatteredpixel.shatteredpixeldungeon.levels.Terrain;
import com.shatteredpixel.shatteredpixeldungeon.levels.painters.CityPainter;
import com.shatteredpixel.shatteredpixeldungeon.levels.painters.Painter;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.Room;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.connection.TunnelRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.StandardRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.*;
import com.shatteredpixel.shatteredpixeldungeon.plants.Plant;
import com.watabou.utils.Random;
import com.watabou.utils.SparseArray;
import com.watabou.noosa.Game;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.HashSet;

/** Isolated official-v3.3.8 map/RNG oracle for the Rust City room module. */
public final class CityRoomPaintOracle {

    private static final long PAINT_SEED = 0x0123456789abcdefL;
    private static final int DEPTH = 18;

    public static final class FixtureLevel extends CityLevel {
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

    public static final class ExposedCityPainter extends CityPainter {
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
        level.setSize(28, 28);
        return level;
    }

    private static void attach(Room room, int size) {
        int left = 5, top = 5, right = left + size - 1, bottom = top + size - 1;
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
                "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.entrance.HallwayEntranceRoom",
                StandardRoom.SizeCategory.NORMAL);
        Room middle = instantiate(
                "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.LibraryHallRoom",
                StandardRoom.SizeCategory.NORMAL);
        Room exit = instantiate(
                "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.exit.StatuesExitRoom",
                StandardRoom.SizeCategory.NORMAL);
        entrance.set(0, 0, 8, 8);
        middle.set(8, 0, 16, 8);
        exit.set(16, 0, 24, 8);
        connect(entrance, middle);
        connect(middle, exit);
        ArrayList<Room> rooms = new ArrayList<>(Arrays.asList(entrance, middle, exit));

        Painter painter = new CityPainter()
                .setWater(0.30f, 4)
                .setGrass(0.20f, 3)
                .setTraps(3,
                        new Class<?>[]{FrostTrap.class, StormTrap.class, CorrosionTrap.class,
                                BlazingTrap.class, DisintegrationTrap.class, RockfallTrap.class,
                                FlashingTrap.class, GuardianTrap.class, WeakeningTrap.class,
                                DisarmingTrap.class, SummoningTrap.class, WarpingTrap.class,
                                CursingTrap.class, PitfallTrap.class, DistortionTrap.class,
                                GatewayTrap.class, GeyserTrap.class},
                        new float[]{4,4,4,4,4, 2,2,2,2, 1,1,1,1,1,1,1,1});

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

    private static void runDecoration() throws Exception {
        FixtureLevel level = level();
        level.setSize(20, 12);
        Dungeon.depth = DEPTH;
        Dungeon.seed = 0;
        Dungeon.level = level;
        Room room = instantiate(
                "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.HallwayRoom",
                StandardRoom.SizeCategory.NORMAL);
        room.set(1, 1, 17, 9);
        Painter.fill(level, room, Terrain.WALL);
        Painter.fill(level, room, 1, Terrain.EMPTY);
        Painter.fill(level, 4, 4, 3, 1, Terrain.BOOKSHELF);
        ArrayList<Room> rooms = new ArrayList<>(Arrays.asList(room));

        Random.pushGenerator(PAINT_SEED);
        new ExposedCityPainter().decorateFixture(level, rooms);
        int next = Random.Int();
        Random.popGenerator();
        System.out.printf("Decoration|%d|%d%n", Arrays.hashCode(level.map), next);
    }

    public static void main(String[] args) throws Exception {
        Game.version = "3.3.8";
        String standard = "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.";
        run("Hallway", standard + "HallwayRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("LibraryHallNormal", standard + "LibraryHallRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("LibraryHallLarge", standard + "LibraryHallRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("LibraryRingNormal", standard + "LibraryRingRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("LibraryRingLarge", standard + "LibraryRingRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("LibraryRingGiant", standard + "LibraryRingRoom", StandardRoom.SizeCategory.GIANT, 16);
        run("StatuesNormal", standard + "StatuesRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("StatuesLarge", standard + "StatuesRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("StatuesGiant", standard + "StatuesRoom", StandardRoom.SizeCategory.GIANT, 17);
        run("SegmentedLibraryLarge", standard + "SegmentedLibraryRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("SegmentedLibraryGiant", standard + "SegmentedLibraryRoom", StandardRoom.SizeCategory.GIANT, 17);

        run("Plants", standard + "PlantsRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("PlantsLarge", standard + "PlantsRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("Aquarium", standard + "AquariumRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("AquariumLarge", standard + "AquariumRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("Platform", standard + "PlatformRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("PlatformLarge", standard + "PlatformRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("PlatformGiant", standard + "PlatformRoom", StandardRoom.SizeCategory.GIANT, 17);
        run("Burned", standard + "BurnedRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("BurnedLarge", standard + "BurnedRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("Fissure", standard + "FissureRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("FissureLarge", standard + "FissureRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("FissureGiant", standard + "FissureRoom", StandardRoom.SizeCategory.GIANT, 17);
        run("GrassyGrave", standard + "GrassyGraveRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("Striped", standard + "StripedRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("StripedLarge", standard + "StripedRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("Study", standard + "StudyRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("StudyLarge", standard + "StudyRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("SuspiciousChest", standard + "SuspiciousChestRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("Minefield", standard + "MinefieldRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("MinefieldLarge", standard + "MinefieldRoom", StandardRoom.SizeCategory.LARGE, 13);

        String entrance = standard + "entrance.";
        String exit = standard + "exit.";
        run("HallwayEntrance", entrance + "HallwayEntranceRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("HallwayExit", exit + "HallwayExitRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("StatuesEntranceNormal", entrance + "StatuesEntranceRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("StatuesEntranceLarge", entrance + "StatuesEntranceRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("StatuesExitNormal", exit + "StatuesExitRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("StatuesExitLarge", exit + "StatuesExitRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("LibraryHallEntranceNormal", entrance + "LibraryHallEntranceRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("LibraryHallEntranceLarge", entrance + "LibraryHallEntranceRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("LibraryHallExitNormal", exit + "LibraryHallExitRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("LibraryHallExitLarge", exit + "LibraryHallExitRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("LibraryRingEntrance", entrance + "LibraryRingEntranceRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("LibraryRingExit", exit + "LibraryRingExitRoom", StandardRoom.SizeCategory.LARGE, 13);

        String connection = "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.connection.";
        run("Perimeter", connection + "PerimeterRoom", null, 9);
        run("Walkway", connection + "WalkwayRoom", null, 9);
        run("RingTunnel", connection + "RingTunnelRoom", null, 9);
        run("RingBridge", connection + "RingBridgeRoom", null, 9);
        run("Maze", connection + "MazeConnectionRoom", null, 9);

        runDecoration();
        runAssembled();
    }
}
