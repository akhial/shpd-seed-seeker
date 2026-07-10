/*
 * Focused v3.3.8 oracle for SpecialRoom.CONSUMABLE_SPECIALS.
 *
 * Each case resets the full run-global state exactly as Dungeon.init does,
 * paints one official class into an isolated level, and emits map/RNG plus
 * normalized typed side-effect lists. Directional variants cover every
 * MagicalFire, Traps, and CrystalPath geometry branch; CrystalPath also uses
 * 7x7, 8x8, and 9x9 bounds to cross all size thresholds.
 */
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.backends.lwjgl3.Lwjgl3Files;
import com.badlogic.gdx.utils.GdxNativesLoader;
import com.shatteredpixel.shatteredpixeldungeon.Dungeon;
import com.shatteredpixel.shatteredpixeldungeon.actors.blobs.Blob;
import com.shatteredpixel.shatteredpixeldungeon.actors.hero.Hero;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Mimic;
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
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.Room;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.CrystalPathRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.GardenRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.LibraryRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.MagicWellRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.MagicalFireRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.RunestoneRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.SpecialRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.StorageRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.ToxicGasRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.TrapsRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.TreasuryRoom;
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

public final class SpecialConsumableOracle {

    private static final class OracleLevel extends Level {
        OracleLevel() {
            setSize(13, 13);
            mobs = new HashSet<>();
            heaps = new SparseArray<>();
            blobs = new HashMap<>();
            plants = new SparseArray<>();
            traps = new SparseArray<>();
            customTiles = new ArrayList<CustomTilemap>();
            customWalls = new ArrayList<CustomTilemap>();
        }

        @Override protected boolean build() { return true; }
        @Override protected void createMobs() {}
        @Override protected void createItems() {}
        List<Item> spawnItems() { return itemsToSpawn; }
    }

    private static final class StubRoom extends Room {
        @Override public void paint(Level level) {}
    }

    private static void initRun(long dungeonSeed) {
        if (Gdx.files == null) {
            GdxNativesLoader.load();
            Gdx.files = new Lwjgl3Files();
        }
        Dungeon.seed = dungeonSeed;
        Dungeon.challenges = 0;
        Random.resetGenerators();
        Random.pushGenerator(dungeonSeed + 1);
        Scroll.initLabels();
        Potion.initColors();
        Ring.initGems();
        SpecialRoom.initForRun();
        SecretRoom.initForRun();
        Generator.fullReset();
        Random.resetGenerators();
        Dungeon.hero = new Hero();
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
        for (Item item : values) result.add(item(item));
        return String.join(",", result);
    }

    private static void printCase(String label, Class<? extends SpecialRoom> roomClass,
                                  int size, int doorX, int doorY) throws Exception {
        printCase(label, roomClass, size, doorX, doorY, 0L);
    }

    private static void printCase(String label, Class<? extends SpecialRoom> roomClass,
                                  int size, int doorX, int doorY, long outerSeed) throws Exception {
        initRun(0L);
        Dungeon.depth = 3;
        OracleLevel level = new OracleLevel();
        Dungeon.level = null;

        Random.pushGenerator(outerSeed);
        SpecialRoom room = roomClass.getDeclaredConstructor().newInstance();
        int right = 2 + size - 1;
        room.set(2, 2, right, right);
        StubRoom neighbour = new StubRoom();
        Room.Door door = new Room.Door(doorX, doorY);
        room.connected.put(neighbour, door);
        neighbour.connected.put(room, door);
        room.paint(level);

        ArrayList<String> heaps = new ArrayList<>();
        for (int cell = 0; cell < level.length(); cell++) {
            Heap heap = level.heaps.get(cell);
            if (heap != null) {
                heaps.add(cell + ":" + heap.type + ":" + heap.autoExplored + ":" + items(heap.items));
            }
        }

        ArrayList<Mob> sortedMobs = new ArrayList<>(level.mobs);
        sortedMobs.sort(Comparator.comparingInt((Mob mob) -> mob.pos)
                .thenComparing(mob -> mob.getClass().getSimpleName()));
        ArrayList<String> mobs = new ArrayList<>();
        for (Mob mob : sortedMobs) {
            String carried = mob instanceof Mimic ? ":" + items(((Mimic)mob).items) : "";
            mobs.add(mob.pos + ":" + mob.getClass().getSimpleName() + carried);
        }

        ArrayList<String> plants = new ArrayList<>();
        for (int cell = 0; cell < level.length(); cell++) {
            Plant plant = level.plants.get(cell);
            if (plant != null) plants.add(cell + ":" + plant.getClass().getSimpleName());
        }

        ArrayList<String> traps = new ArrayList<>();
        for (int cell = 0; cell < level.length(); cell++) {
            Trap trap = level.traps.get(cell);
            if (trap != null) {
                traps.add(cell + ":" + trap.getClass().getSimpleName()
                        + ":" + trap.visible + ":" + trap.active);
            }
        }

        ArrayList<String> blobs = new ArrayList<>();
        for (Blob blob : level.blobs.values()) {
            blobs.add(blob.getClass().getSimpleName() + ":" + blob.volume
                    + ":" + Arrays.hashCode(blob.cur));
        }
        blobs.sort(String::compareTo);

        String eventText = "heaps=[" + String.join(";", heaps)
                + "]|mobs=[" + String.join(";", mobs)
                + "]|spawn=[" + items(level.spawnItems())
                + "]|plants=[" + String.join(";", plants)
                + "]|traps=[" + String.join(";", traps)
                + "]|blobs=[" + String.join(";", blobs) + "]";
        System.out.printf(
                "%s|map=%d|door=%s|%s|events=%d|next=%d%n",
                label, Arrays.hashCode(level.map), door.type, eventText,
                eventText.hashCode(), Random.Long());
        Random.popGenerator();
    }

    public static void main(String[] args) throws Exception {
        printCase("Runestone-L7", RunestoneRoom.class, 7, 2, 5);
        printCase("Garden-L7", GardenRoom.class, 7, 2, 5);
        printCase("Library-L7", LibraryRoom.class, 7, 2, 5);
        printCase("Storage-L7", StorageRoom.class, 7, 2, 5);
        printCase("Treasury-L7", TreasuryRoom.class, 7, 2, 5);
        printCase("MagicWell-L7", MagicWellRoom.class, 7, 2, 5);
        printCase("ToxicGas-L7", ToxicGasRoom.class, 7, 2, 5);
        printCase("MagicalFire-L7", MagicalFireRoom.class, 7, 2, 5);
        printCase("Traps-L7", TrapsRoom.class, 7, 2, 5);
        printCase("CrystalPath-L7", CrystalPathRoom.class, 7, 2, 5);

        printCase("MagicalFire-R7", MagicalFireRoom.class, 7, 8, 5);
        printCase("MagicalFire-T7", MagicalFireRoom.class, 7, 5, 2);
        printCase("MagicalFire-B7", MagicalFireRoom.class, 7, 5, 8);
        printCase("Traps-R7", TrapsRoom.class, 7, 8, 5);
        printCase("Traps-T7", TrapsRoom.class, 7, 5, 2);
        printCase("Traps-B7", TrapsRoom.class, 7, 5, 8);
        printCase("CrystalPath-R8", CrystalPathRoom.class, 8, 9, 5);
        printCase("CrystalPath-T9", CrystalPathRoom.class, 9, 6, 2);
        printCase("CrystalPath-B7", CrystalPathRoom.class, 7, 5, 8);
        printCase("Treasury-Mimic-L7", TreasuryRoom.class, 7, 2, 5, 4L);
    }
}
