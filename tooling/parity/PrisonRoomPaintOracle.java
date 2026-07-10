import com.shatteredpixel.shatteredpixeldungeon.Dungeon;
import com.shatteredpixel.shatteredpixeldungeon.items.Generator;
import com.shatteredpixel.shatteredpixeldungeon.items.Gold;
import com.shatteredpixel.shatteredpixeldungeon.items.Heap;
import com.shatteredpixel.shatteredpixeldungeon.items.Item;
import com.shatteredpixel.shatteredpixeldungeon.levels.Level;
import com.shatteredpixel.shatteredpixeldungeon.levels.PrisonLevel;
import com.shatteredpixel.shatteredpixeldungeon.levels.Terrain;
import com.shatteredpixel.shatteredpixeldungeon.levels.features.LevelTransition;
import com.shatteredpixel.shatteredpixeldungeon.levels.painters.PrisonPainter;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.Room;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.connection.TunnelRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.StandardRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.Trap;
import com.shatteredpixel.shatteredpixeldungeon.plants.Plant;
import com.watabou.utils.Random;
import com.watabou.utils.SparseArray;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.HashSet;

/** Isolated map/RNG oracle for the Rust PrisonRoomDispatcher unit graph. */
public final class PrisonRoomPaintOracle {

    private static final long PAINT_SEED = 0x0123456789abcdefL;

    public static final class FixtureLevel extends PrisonLevel {
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

    public static final class ExposedPrisonPainter extends PrisonPainter {
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
        Dungeon.depth = 8;
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

    private static void fill(Level level, int left, int top, int right, int bottom, int tile) {
        for (int y = top; y <= bottom; y++) {
            for (int x = left; x <= right; x++) {
                level.map[x + y * level.width()] = tile;
            }
        }
    }

    private static void runDecoration() throws Exception {
        FixtureLevel level = level();
        Arrays.fill(level.map, Terrain.WALL);
        Dungeon.depth = 8;
        Dungeon.level = level;

        String standard = "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.";
        Room fissure = instantiate(standard + "FissureRoom", StandardRoom.SizeCategory.NORMAL);
        Room bridge = instantiate(standard + "ChasmBridgeRoom", StandardRoom.SizeCategory.NORMAL);
        Room special = instantiate(
                "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.StorageRoom", null);
        fissure.set(2, 2, 10, 10);
        bridge.set(11, 2, 20, 10);
        special.set(2, 11, 10, 20);

        fill(level, 3, 3, 9, 9, Terrain.EMPTY);
        fill(level, 12, 3, 19, 9, Terrain.EMPTY_SP);
        fill(level, 3, 12, 9, 19, Terrain.EMPTY);
        fill(level, 4, 4, 8, 8, Terrain.CHASM);
        fill(level, 13, 4, 18, 8, Terrain.CHASM);
        fill(level, 4, 13, 8, 18, Terrain.CHASM);
        ArrayList<Room> rooms = new ArrayList<>(Arrays.asList(fissure, bridge, special));

        Random.pushGenerator(PAINT_SEED);
        new ExposedPrisonPainter().decorateFixture(level, rooms);
        int next = Random.Int();
        Random.popGenerator();
        System.out.printf("Decoration|%d|%d%n", Arrays.hashCode(level.map), next);
    }

    private static void runAssembled() throws Exception {
        FixtureLevel level = level();
        level.map = new int[0];
        Dungeon.depth = 8;
        Dungeon.level = level;

        String standard = "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.";
        Room entrance = instantiate(
                standard + "entrance.RegionDecoLineEntranceRoom",
                StandardRoom.SizeCategory.NORMAL);
        Room middle = instantiate(standard + "FissureRoom", StandardRoom.SizeCategory.NORMAL);
        Room exit = instantiate(
                standard + "exit.CellBlockExitRoom",
                StandardRoom.SizeCategory.LARGE);
        entrance.set(0, 0, 8, 8);
        middle.set(8, 0, 16, 8);
        exit.set(16, 0, 28, 12);
        connect(entrance, middle);
        connect(middle, exit);
        ArrayList<Room> rooms = new ArrayList<>(Arrays.asList(entrance, middle, exit));

        Random.pushGenerator(PAINT_SEED);
        boolean painted = new PrisonPainter().paint(level, rooms);
        int next = Random.Int();
        Random.popGenerator();
        String order = rooms.stream()
                .map(room -> room.getClass().getSimpleName())
                .reduce((left, right) -> left + "," + right)
                .orElse("");
        System.out.printf("Assembled|%s|%d|%d|%d|%s%n",
                painted, Arrays.hashCode(level.map), next, level.transitions.size(), order);
    }

    public static void main(String[] args) throws Exception {
        String standard = "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.";
        run("RegionDecoLine", standard + "RegionDecoLineRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("Segmented", standard + "SegmentedRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("Pillars", standard + "PillarsRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("ChasmBridge", standard + "ChasmBridgeRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("CellBlock", standard + "CellBlockRoom", StandardRoom.SizeCategory.LARGE, 13);

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
        run("RegionLineEntrance", entrance + "RegionDecoLineEntranceRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("RegionLineExit", exit + "RegionDecoLineExitRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("ChasmBridgeEntrance", entrance + "ChasmBridgeEntranceRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("ChasmBridgeExit", exit + "ChasmBridgeExitRoom", StandardRoom.SizeCategory.NORMAL, 9);
        run("PillarsEntrance", entrance + "PillarsEntranceRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("PillarsExit", exit + "PillarsExitRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("CellBlockEntrance", entrance + "CellBlockEntranceRoom", StandardRoom.SizeCategory.LARGE, 13);
        run("CellBlockExit", exit + "CellBlockExitRoom", StandardRoom.SizeCategory.LARGE, 13);

        String connection = "com.shatteredpixel.shatteredpixeldungeon.levels.rooms.connection.";
        run("Perimeter", connection + "PerimeterRoom", null, 9);
        run("Walkway", connection + "WalkwayRoom", null, 9);
        run("Maze", connection + "MazeConnectionRoom", null, 9);
        runDecoration();
        runAssembled();
    }
}
