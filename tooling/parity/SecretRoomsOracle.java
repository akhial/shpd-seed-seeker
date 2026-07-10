/*
 * Focused official-v3.3.8 oracle for all regular SecretRoom painters.
 *
 * Laboratory and Library deliberately report the iteration order of the
 * copied HashMap<Class,...> used by Random.chances. Class identity hashes make
 * that order runtime-specific; the Rust port therefore treats it as explicit
 * runtime profile data rather than silently sorting it.
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
import com.shatteredpixel.shatteredpixeldungeon.items.armor.Armor;
import com.shatteredpixel.shatteredpixeldungeon.items.potions.Potion;
import com.shatteredpixel.shatteredpixeldungeon.items.rings.Ring;
import com.shatteredpixel.shatteredpixeldungeon.items.scrolls.Scroll;
import com.shatteredpixel.shatteredpixeldungeon.items.weapon.Weapon;
import com.shatteredpixel.shatteredpixeldungeon.levels.Level;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.Room;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretArtilleryRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretChestChasmRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretGardenRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretHoardRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretHoneypotRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretLaboratoryRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretLarderRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretLibraryRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretMazeRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretRunestoneRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretSummoningRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretWellRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.SpecialRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.Trap;
import com.shatteredpixel.shatteredpixeldungeon.plants.Plant;
import com.shatteredpixel.shatteredpixeldungeon.tiles.CustomTilemap;
import com.watabou.utils.Random;
import com.watabou.utils.SparseArray;

import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;

public final class SecretRoomsOracle {

    private static final class OracleLevel extends Level {
        OracleLevel() {
            setSize(25, 25);
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
        Dungeon.depth = 3;
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

    private static void printHashProfile(String label, Class<?> owner, String fieldName)
            throws Exception {
        Field field = owner.getDeclaredField(fieldName);
        field.setAccessible(true);
        @SuppressWarnings("unchecked")
        HashMap<Class<?>, Float> original = (HashMap<Class<?>, Float>)field.get(null);
        HashMap<Class<?>, Float> copied = new HashMap<>(original);
        ArrayList<String> classes = new ArrayList<>();
        for (Class<?> key : copied.keySet()) classes.add(key.getSimpleName());
        System.out.printf("profile|%s|java=%s|vendor=%s|vm=%s|order=%s%n",
                label, System.getProperty("java.version"), System.getProperty("java.vendor"),
                System.getProperty("java.vm.name"), String.join(",", classes));
    }

    private static void printCase(String label, Class<? extends SecretRoom> roomClass,
                                  int size, int doorX, int doorY) throws Exception {
        initRun(0L);
        OracleLevel level = new OracleLevel();
        Dungeon.level = null;

        Random.pushGenerator(0L);
        SecretRoom room = roomClass.getDeclaredConstructor().newInstance();
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
                heaps.add(cell + ":" + heap.type + ":" + heap.haunted + ":" + items(heap.items));
            }
        }

        ArrayList<Mob> sortedMobs = new ArrayList<>(level.mobs);
        sortedMobs.sort(Comparator.comparingInt((Mob mob) -> mob.pos)
                .thenComparing(mob -> mob.getClass().getSimpleName()));
        ArrayList<String> mobs = new ArrayList<>();
        for (Mob mob : sortedMobs) mobs.add(mob.pos + ":" + mob.getClass().getSimpleName());

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
        System.out.printf("%s|map=%d|door=%s|%s|events=%d|next=%d%n",
                label, Arrays.hashCode(level.map), door.type, eventText,
                eventText.hashCode(), Random.Long());
        Random.popGenerator();
    }

    public static void main(String[] args) throws Exception {
        printHashProfile("laboratory", SecretLaboratoryRoom.class, "potionChances");
        printHashProfile("library", SecretLibraryRoom.class, "scrollChances");

        printCase("Garden-L7", SecretGardenRoom.class, 7, 2, 5);
        printCase("Laboratory-L7", SecretLaboratoryRoom.class, 7, 2, 5);
        printCase("Library-L7", SecretLibraryRoom.class, 7, 2, 5);
        printCase("Library-T7", SecretLibraryRoom.class, 7, 5, 2);
        printCase("Larder-L7", SecretLarderRoom.class, 7, 2, 5);
        printCase("Well-L7", SecretWellRoom.class, 7, 2, 5);
        printCase("Well-R7", SecretWellRoom.class, 7, 8, 5);
        printCase("Well-T7", SecretWellRoom.class, 7, 5, 2);
        printCase("Well-B7", SecretWellRoom.class, 7, 5, 8);
        printCase("Runestone-L7", SecretRunestoneRoom.class, 7, 2, 5);
        printCase("Runestone-R7", SecretRunestoneRoom.class, 7, 8, 5);
        printCase("Runestone-T7", SecretRunestoneRoom.class, 7, 5, 2);
        printCase("Runestone-B7", SecretRunestoneRoom.class, 7, 5, 8);
        printCase("Artillery-L7", SecretArtilleryRoom.class, 7, 2, 5);
        printCase("ChestChasm-L8", SecretChestChasmRoom.class, 8, 2, 5);
        printCase("Honeypot-L7", SecretHoneypotRoom.class, 7, 2, 5);
        printCase("Hoard-L7", SecretHoardRoom.class, 7, 2, 5);
        printCase("Maze-L14", SecretMazeRoom.class, 14, 2, 8);
        printCase("Summoning-L7", SecretSummoningRoom.class, 7, 2, 5);
    }
}
