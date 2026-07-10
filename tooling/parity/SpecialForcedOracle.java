/*
 * Focused v3.3.8 oracle for LaboratoryRoom, PitRoom, and ShopRoom.
 *
 * Every case calls the official painter directly with a fixed inclusive room
 * rectangle.  Laboratory guide state and itemsToSpawn are explicit, Pit also
 * exercises RegularLevel.fallCell(true), and Shop prewarms spacesNeeded just
 * as room sizing does before painting.
 */
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.backends.lwjgl3.Lwjgl3Files;
import com.badlogic.gdx.utils.GdxNativesLoader;
import com.shatteredpixel.shatteredpixeldungeon.Dungeon;
import com.shatteredpixel.shatteredpixeldungeon.actors.blobs.Blob;
import com.shatteredpixel.shatteredpixeldungeon.actors.hero.Hero;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Mob;
import com.shatteredpixel.shatteredpixeldungeon.items.Generator;
import com.shatteredpixel.shatteredpixeldungeon.items.Heap;
import com.shatteredpixel.shatteredpixeldungeon.items.Item;
import com.shatteredpixel.shatteredpixeldungeon.items.Torch;
import com.shatteredpixel.shatteredpixeldungeon.items.armor.Armor;
import com.shatteredpixel.shatteredpixeldungeon.items.bags.Bag;
import com.shatteredpixel.shatteredpixeldungeon.items.journal.DocumentPage;
import com.shatteredpixel.shatteredpixeldungeon.items.potions.Potion;
import com.shatteredpixel.shatteredpixeldungeon.items.potions.PotionOfStrength;
import com.shatteredpixel.shatteredpixeldungeon.items.rings.Ring;
import com.shatteredpixel.shatteredpixeldungeon.items.scrolls.Scroll;
import com.shatteredpixel.shatteredpixeldungeon.items.trinkets.TrinketCatalyst;
import com.shatteredpixel.shatteredpixeldungeon.items.weapon.Weapon;
import com.shatteredpixel.shatteredpixeldungeon.journal.Document;
import com.shatteredpixel.shatteredpixeldungeon.levels.Level;
import com.shatteredpixel.shatteredpixeldungeon.levels.RegularLevel;
import com.shatteredpixel.shatteredpixeldungeon.levels.Terrain;
import com.shatteredpixel.shatteredpixeldungeon.levels.painters.Painter;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.Room;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.LaboratoryRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.PitRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.ShopRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.SpecialRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.Trap;
import com.shatteredpixel.shatteredpixeldungeon.plants.Plant;
import com.shatteredpixel.shatteredpixeldungeon.tiles.CustomTilemap;
import com.watabou.utils.Random;
import com.watabou.utils.SparseArray;

import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;

public final class SpecialForcedOracle {

    private static final class OracleLevel extends RegularLevel {
        private final ArrayList<Integer> dropCells = new ArrayList<>();

        OracleLevel() {
            setSize(13, 13);
            mobs = new HashSet<>();
            heaps = new SparseArray<>();
            blobs = new HashMap<>();
            plants = new SparseArray<Plant>();
            traps = new SparseArray<Trap>();
            customTiles = new ArrayList<CustomTilemap>();
            customWalls = new ArrayList<CustomTilemap>();
        }

        @Override protected boolean build() { return true; }
        @Override protected void createMobs() {}
        @Override protected void createItems() {}
        @Override protected Painter painter() { return null; }

        @Override public Heap drop(Item item, int cell) {
            Heap result = super.drop(item, cell);
            dropCells.add(cell);
            return result;
        }

        void queue(Item item) { addItemToSpawn(item); }
        List<Item> queued() { return itemsToSpawn; }
        void rooms(Room... value) { rooms = new ArrayList<>(Arrays.asList(value)); }
        List<Integer> dropCells() { return dropCells; }
    }

    private static final class StubRoom extends Room {
        @Override public void paint(Level level) {}
    }

    private static final class InjectedShopRoom extends ShopRoom {
        void injectTorches(int count) {
            itemsToSpawn = new ArrayList<>();
            for (int i = 0; i < count; i++) itemsToSpawn.add(new Torch());
        }
    }

    private static final Field PAGE_STATES;

    static {
        try {
            PAGE_STATES = Document.class.getDeclaredField("pagesStates");
            PAGE_STATES.setAccessible(true);
        } catch (ReflectiveOperationException error) {
            throw new ExceptionInInitializerError(error);
        }
    }

    private static void initRun(long dungeonSeed) {
        if (Gdx.files == null) {
            GdxNativesLoader.load();
            Gdx.files = new Lwjgl3Files();
        }
        Dungeon.seed = dungeonSeed;
        Dungeon.challenges = 0;
        Dungeon.branch = 0;

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
        Dungeon.LimitedDrops.VELVET_POUCH.drop();
        Dungeon.hero = new Hero();
    }

    @SuppressWarnings("unchecked")
    private static void setAlchemyPages(boolean found) throws Exception {
        LinkedHashMap<String, Integer> states =
                (LinkedHashMap<String, Integer>)PAGE_STATES.get(Document.ALCHEMY_GUIDE);
        for (String page : states.keySet()) {
            states.put(page, found ? Document.READ : Document.NOT_FOUND);
        }
    }

    private static Room.Door connect(SpecialRoom room, int x, int y) {
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
        String type = item instanceof Bag ? "Bag" : item.getClass().getSimpleName();
        String page = item instanceof DocumentPage ? ":p" + ((DocumentPage)item).page() : "";
        return String.format("%s:q%d:l%d:c%s:e%s%s",
                type, item.quantity(), item.level(), item.cursed, effect(item), page);
    }

    private static String items(Iterable<Item> values) {
        ArrayList<String> result = new ArrayList<>();
        for (Item value : values) result.add(item(value));
        return String.join(",", result);
    }

    private static String heaps(OracleLevel level) {
        ArrayList<String> result = new ArrayList<>();
        for (int cell = 0; cell < level.length(); cell++) {
            Heap heap = level.heaps.get(cell);
            if (heap != null) {
                result.add(cell + ":" + heap.type + ":h" + heap.haunted + ":" + items(heap.items));
            }
        }
        return String.join(";", result);
    }

    private static String terrainCells(OracleLevel level, int terrain) {
        ArrayList<String> result = new ArrayList<>();
        for (int cell = 0; cell < level.length(); cell++) {
            if (level.map[cell] == terrain) result.add(Integer.toString(cell));
        }
        return String.join(",", result);
    }

    private static void laboratory(long outerSeed, boolean queued, boolean pagesMissing)
            throws Exception {
        initRun(0);
        Dungeon.depth = 6;
        setAlchemyPages(!pagesMissing);
        OracleLevel level = new OracleLevel();
        Dungeon.level = null;
        if (queued) {
            level.queue(new TrinketCatalyst());
            level.queue(new PotionOfStrength());
        }

        Random.pushGenerator(outerSeed);
        LaboratoryRoom room = new LaboratoryRoom();
        room.set(2, 2, 8, 8);
        Room.Door door = connect(room, 2, 5);
        room.paint(level);
        long next = Random.Long();
        Random.popGenerator();

        Blob alchemy = level.blobs.get(
                com.shatteredpixel.shatteredpixeldungeon.actors.blobs.Alchemy.class);
        System.out.printf(
                "Laboratory seed=%d queued=%s missing=%s map=%d door=%s alchemy=%s:v%d heaps=[%s] spawn=[%s] next=%d%n",
                outerSeed, queued, pagesMissing, Arrays.hashCode(level.map), door.type,
                terrainCells(level, Terrain.ALCHEMY), alchemy == null ? 0 : alchemy.volume,
                heaps(level), items(level.queued()), next);
    }

    private static void pit(long outerSeed, long fallSeed) throws Exception {
        initRun(0);
        Dungeon.depth = 11;
        setAlchemyPages(true);
        OracleLevel level = new OracleLevel();
        Dungeon.level = null;

        Random.pushGenerator(outerSeed);
        PitRoom room = new PitRoom();
        room.set(2, 2, 8, 8);
        Room.Door door = connect(room, 2, 5);
        level.rooms(room);
        room.paint(level);
        long next = Random.Long();
        Random.popGenerator();

        level.buildFlagMaps();
        Random.pushGenerator(fallSeed);
        int fall = level.fallCell(true);
        long fallNext = Random.Long();
        Random.popGenerator();

        System.out.printf(
                "Pit seed=%d map=%d door=%s well=%s heaps=[%s] fallSeed=%d fall=%d fallNext=%d next=%d%n",
                outerSeed, Arrays.hashCode(level.map), door.type,
                terrainCells(level, Terrain.EMPTY_WELL), heaps(level), fallSeed, fall, fallNext, next);
    }

    private static void shop(long outerSeed) throws Exception {
        initRun(0);
        Dungeon.depth = 6;
        OracleLevel level = new OracleLevel();
        Dungeon.level = null;

        Random.pushGenerator(outerSeed);
        ShopRoom room = new ShopRoom();
        int minimum = room.minWidth();
        room.set(2, 2, 9, 9);
        Room.Door door = connect(room, 2, 5);
        room.paint(level);
        long next = Random.Long();
        Random.popGenerator();

        ArrayList<String> mobs = new ArrayList<>();
        for (Mob mob : level.mobs) mobs.add(mob.pos + ":" + mob.getClass().getSimpleName());
        mobs.sort(String::compareTo);
        System.out.printf(
                "Shop seed=%d min=%d map=%d door=%s heaps=[%s] mobs=[%s] next=%d%n",
                outerSeed, minimum, Arrays.hashCode(level.map), door.type, heaps(level),
                String.join(";", mobs), next);
    }

    private static void shopOverflow(long outerSeed) throws Exception {
        initRun(0);
        Dungeon.depth = 6;
        OracleLevel level = new OracleLevel();
        Dungeon.level = null;

        Random.pushGenerator(outerSeed);
        InjectedShopRoom room = new InjectedShopRoom();
        room.injectTorches(24);
        room.set(0, 0, 6, 6);
        Room.Door door = connect(room, 0, 3);
        room.paint(level);
        long next = Random.Long();
        Random.popGenerator();

        System.out.printf(
                "ShopOverflow seed=%d map=%d door=%s drops=%s next=%d%n",
                outerSeed, Arrays.hashCode(level.map), door.type, level.dropCells(), next);
    }

    public static void main(String[] args) throws Exception {
        laboratory(123L, false, false);
        laboratory(4L, true, false);
        laboratory(9876L, false, true);
        pit(123L, 77L);
        shop(123L);
        shopOverflow(55L);
    }
}
