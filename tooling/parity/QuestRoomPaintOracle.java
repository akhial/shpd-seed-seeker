/*
 * Focused official-v3.3.8 oracle for the five regular quest rooms and the
 * mandatory Halls DemonSpawnerRoom.  Room construction happens before the
 * fixed paint stream, because the two StandardRoom subclasses consume their
 * size-category draw during the much earlier initRooms phase.
 */
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.backends.lwjgl3.Lwjgl3Files;
import com.badlogic.gdx.utils.GdxNativesLoader;
import com.shatteredpixel.shatteredpixeldungeon.Dungeon;
import com.shatteredpixel.shatteredpixeldungeon.Statistics;
import com.shatteredpixel.shatteredpixeldungeon.actors.hero.Hero;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.DemonSpawner;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Mob;
import com.shatteredpixel.shatteredpixeldungeon.items.Generator;
import com.shatteredpixel.shatteredpixeldungeon.items.Heap;
import com.shatteredpixel.shatteredpixeldungeon.items.Item;
import com.shatteredpixel.shatteredpixeldungeon.items.armor.Armor;
import com.shatteredpixel.shatteredpixeldungeon.items.potions.Potion;
import com.shatteredpixel.shatteredpixeldungeon.items.rings.Ring;
import com.shatteredpixel.shatteredpixeldungeon.items.scrolls.Scroll;
import com.shatteredpixel.shatteredpixeldungeon.items.weapon.Weapon;
import com.shatteredpixel.shatteredpixeldungeon.levels.Level;
import com.shatteredpixel.shatteredpixeldungeon.levels.RegularLevel;
import com.shatteredpixel.shatteredpixeldungeon.levels.Terrain;
import com.shatteredpixel.shatteredpixeldungeon.levels.features.LevelTransition;
import com.shatteredpixel.shatteredpixeldungeon.levels.painters.Painter;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.Room;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.quest.AmbitiousImpRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.quest.BlacksmithRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.quest.MassGraveRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.quest.RitualSiteRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.quest.RotGardenRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.DemonSpawnerRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.SpecialRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.Trap;
import com.shatteredpixel.shatteredpixeldungeon.plants.Plant;
import com.shatteredpixel.shatteredpixeldungeon.tiles.CustomTilemap;
import com.watabou.utils.Random;
import com.watabou.utils.SparseArray;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;

public final class QuestRoomPaintOracle {

    private static final class OracleLevel extends RegularLevel {
        OracleLevel() {
            setSize(15, 15);
            mobs = new HashSet<>();
            heaps = new SparseArray<>();
            blobs = new HashMap<>();
            plants = new SparseArray<Plant>();
            traps = new SparseArray<Trap>();
            transitions = new ArrayList<LevelTransition>();
            customTiles = new ArrayList<CustomTilemap>();
            customWalls = new ArrayList<CustomTilemap>();
        }

        @Override protected boolean build() { return true; }
        @Override protected void createMobs() {}
        @Override protected void createItems() {}
        @Override protected Painter painter() { return null; }
        List<Item> queued() { return itemsToSpawn; }
    }

    private static final class StubRoom extends Room {
        @Override public void paint(Level level) {}
    }

    private static void initRun(long dungeonSeed, int depth) {
        if (Gdx.files == null) {
            GdxNativesLoader.load();
            Gdx.files = new Lwjgl3Files();
        }
        Dungeon.seed = dungeonSeed;
        Dungeon.depth = depth;
        Dungeon.branch = 0;
        Dungeon.challenges = 0;
        Dungeon.hero = new Hero();
        Statistics.spawnersAlive = 0;
        Statistics.amuletObtained = false;

        Random.resetGenerators();
        Random.pushGenerator(dungeonSeed + 1);
        Scroll.initLabels();
        Potion.initColors();
        Ring.initGems();
        SpecialRoom.initForRun();
        SecretRoom.initForRun();
        Generator.fullReset();
        Random.resetGenerators();
    }

    private static Room.Door connect(Room room, int x, int y) {
        StubRoom neighbour = new StubRoom();
        Room.Door door = new Room.Door(x, y);
        room.connected.put(neighbour, door);
        neighbour.connected.put(room, door);
        return door;
    }

    private static String effect(Item item) {
        if (item instanceof Weapon && ((Weapon)item).enchantment != null) {
            return ((Weapon)item).enchantment.getClass().getSimpleName();
        }
        if (item instanceof Armor && ((Armor)item).glyph != null) {
            return ((Armor)item).glyph.getClass().getSimpleName();
        }
        return "none";
    }

    private static String item(Item item) {
        return String.format("%s:+%d:%s:%s:q%d",
                item.getClass().getSimpleName(), item.level(),
                item.cursed ? "cursed" : "clean", effect(item), item.quantity());
    }

    private static String items(Iterable<Item> values) {
        ArrayList<String> result = new ArrayList<>();
        for (Item value : values) result.add(item(value));
        return String.join(",", result);
    }

    private static void printCase(String label, Class<? extends Room> roomClass,
                                  int depth, long outerSeed, int size,
                                  int doorX, int doorY) throws Exception {
        initRun(0L, depth);
        OracleLevel level = new OracleLevel();
        Dungeon.level = null;

        // Constructor RNG belongs to initRooms, not this isolated paint stream.
        Room room = roomClass.getDeclaredConstructor().newInstance();
        int right = 2 + size - 1;
        room.set(2, 2, right, right);
        Room.Door door = connect(room, doorX, doorY);

        Random.pushGenerator(outerSeed);
        room.paint(level);
        long next = Random.Long();
        Random.popGenerator();

        ArrayList<String> heaps = new ArrayList<>();
        for (int cell = 0; cell < level.length(); cell++) {
            Heap heap = level.heaps.get(cell);
            if (heap != null) {
                heaps.add(cell + ":" + heap.type + ":h" + heap.haunted + ":" + items(heap.items));
            }
        }

        ArrayList<Mob> sortedMobs = new ArrayList<>(level.mobs);
        sortedMobs.sort(Comparator.comparingInt((Mob mob) -> mob.pos)
                .thenComparing(mob -> mob.getClass().getSimpleName()));
        ArrayList<String> mobs = new ArrayList<>();
        for (Mob mob : sortedMobs) {
            String suffix = mob instanceof DemonSpawner
                    ? ":recorded=" + ((DemonSpawner)mob).spawnRecorded : "";
            mobs.add(mob.pos + ":" + mob.getClass().getSimpleName() + suffix);
        }

        ArrayList<String> traps = new ArrayList<>();
        for (int cell = 0; cell < level.length(); cell++) {
            Trap trap = level.traps.get(cell);
            if (trap != null) {
                traps.add(cell + ":" + trap.getClass().getSimpleName()
                        + ":" + trap.visible + ":" + trap.active);
            }
        }

        ArrayList<String> custom = new ArrayList<>();
        for (CustomTilemap tile : level.customTiles) {
            custom.add(tile.getClass().getSimpleName() + "@" + tile.tileX + "," + tile.tileY
                    + ":" + tile.tileW + "x" + tile.tileH);
        }
        custom.sort(String::compareTo);

        ArrayList<String> transitions = new ArrayList<>();
        for (LevelTransition transition : level.transitions) {
            transitions.add(transition.cell() + ":" + transition.type);
        }

        String eventText = "heaps=[" + String.join(";", heaps)
                + "]|mobs=[" + String.join(";", mobs)
                + "]|spawn=[" + items(level.queued())
                + "]|traps=[" + String.join(";", traps)
                + "]|custom=[" + String.join(";", custom)
                + "]|transitions=[" + String.join(";", transitions) + "]";
        System.out.printf(
                "%s|map=%d|door=%s|%s|events=%d|spawners=%d|next=%d%n",
                label, Arrays.hashCode(level.map), door.type, eventText,
                eventText.hashCode(), Statistics.spawnersAlive, next);
    }

    public static void main(String[] args) throws Exception {
        printCase("MassGrave-s0", MassGraveRoom.class, 8, 0L, 10, 2, 6);
        printCase("MassGrave-s19", MassGraveRoom.class, 8, 19L, 10, 2, 6);
        printCase("RitualSite-s0", RitualSiteRoom.class, 8, 0L, 10, 2, 6);
        printCase("RotGarden-s0", RotGardenRoom.class, 8, 0L, 10, 2, 6);
        printCase("RotGarden-s31", RotGardenRoom.class, 8, 31L, 10, 2, 6);
        printCase("Blacksmith-s0", BlacksmithRoom.class, 13, 0L, 10, 2, 6);
        printCase("Blacksmith-s77", BlacksmithRoom.class, 13, 77L, 10, 2, 6);
        printCase("AmbitiousImp-L", AmbitiousImpRoom.class, 18, 0L, 9, 2, 6);
        printCase("AmbitiousImp-R", AmbitiousImpRoom.class, 18, 0L, 9, 10, 6);
        printCase("AmbitiousImp-T", AmbitiousImpRoom.class, 18, 0L, 9, 6, 2);
        printCase("AmbitiousImp-B", AmbitiousImpRoom.class, 18, 0L, 9, 6, 10);
        printCase("DemonSpawner-s0", DemonSpawnerRoom.class, 22, 0L, 7, 2, 5);
        printCase("DemonSpawner-s9", DemonSpawnerRoom.class, 22, 9L, 8, 2, 5);
    }
}
