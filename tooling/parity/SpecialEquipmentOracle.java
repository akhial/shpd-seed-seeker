/*
 * Focused v3.3.8 oracle for the nine equipment-heavy SpecialRoom painters.
 *
 * This helper calls the official room classes directly.  It deliberately uses
 * an empty Level.itemsToSpawn list so Pool/Sentry exercise their generated
 * fallback prizes and Armory has no optional catalyst.  Each case resets the
 * run-global Generator decks, fixes a 7x7 room at (2,2)..(8,8), and places the
 * sole entrance at (2,4).  Output is stable, compact, and intended to be pinned
 * by the Rust module tests rather than consumed by production code.
 */
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.backends.lwjgl3.Lwjgl3Files;
import com.badlogic.gdx.utils.GdxNativesLoader;
import com.shatteredpixel.shatteredpixeldungeon.Dungeon;
import com.shatteredpixel.shatteredpixeldungeon.actors.blobs.Blob;
import com.shatteredpixel.shatteredpixeldungeon.actors.blobs.SacrificialFire;
import com.shatteredpixel.shatteredpixeldungeon.actors.hero.Hero;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.ArmoredStatue;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.CrystalMimic;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Mimic;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Mob;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Statue;
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
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.ArmoryRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.CryptRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.CrystalChoiceRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.CrystalVaultRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.PoolRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.SacrificeRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.SentryRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.SpecialRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.StatueRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.WeakFloorRoom;
import com.shatteredpixel.shatteredpixeldungeon.plants.Plant;
import com.shatteredpixel.shatteredpixeldungeon.tiles.CustomTilemap;
import com.shatteredpixel.shatteredpixeldungeon.levels.traps.Trap;
import com.watabou.utils.Random;
import com.watabou.utils.SparseArray;

import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;

public final class SpecialEquipmentOracle {

    private static final class OracleLevel extends Level {
        OracleLevel() {
            setSize(11, 11);
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

        List<Item> spawnItems() { return itemsToSpawn; }
    }

    private static final class StubRoom extends Room {
        @Override public void paint(Level level) {}
    }

    private static final Field SACRIFICE_PRIZE;

    static {
        try {
            SACRIFICE_PRIZE = SacrificialFire.class.getDeclaredField("prize");
            SACRIFICE_PRIZE.setAccessible(true);
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

    private static String items(List<Item> items) {
        ArrayList<String> result = new ArrayList<>();
        for (Item item : items) result.add(item(item));
        return String.join(",", result);
    }

    private static void printCase(Class<? extends SpecialRoom> roomClass,
                                  int depth, long dungeonSeed, long outerSeed) throws Exception {
        initRun(dungeonSeed);
        Dungeon.depth = depth;
        OracleLevel level = new OracleLevel();
        // Keep the global null so Level.drop does not attempt to notify an
        // absent GameScene; all painter logic receives this level explicitly.
        Dungeon.level = null;

        Random.pushGenerator(outerSeed);
        SpecialRoom room = roomClass.getDeclaredConstructor().newInstance();
        room.set(2, 2, 8, 8);
        StubRoom neighbour = new StubRoom();
        Room.Door door = new Room.Door(2, 4);
        room.connected.put(neighbour, door);
        neighbour.connected.put(room, door);
        room.paint(level);

        ArrayList<String> heaps = new ArrayList<>();
        for (int cell = 0; cell < level.length(); cell++) {
            Heap heap = level.heaps.get(cell);
            if (heap != null) {
                heaps.add(cell + ":" + heap.type + ":" + items(heap.items));
            }
        }

        ArrayList<Mob> sortedMobs = new ArrayList<>(level.mobs);
        sortedMobs.sort(Comparator.comparingInt((Mob mob) -> mob.pos)
                .thenComparing(mob -> mob.getClass().getSimpleName()));
        ArrayList<String> mobs = new ArrayList<>();
        for (Mob mob : sortedMobs) {
            String carried = "";
            if (mob instanceof Mimic) {
                carried = ":" + items(((Mimic)mob).items);
            } else if (mob instanceof Statue) {
                ArrayList<Item> statueItems = new ArrayList<>();
                statueItems.add(((Statue)mob).weapon());
                if (mob instanceof ArmoredStatue) {
                    statueItems.add(((ArmoredStatue)mob).armor());
                }
                carried = ":" + items(statueItems);
            }
            mobs.add(mob.pos + ":" + mob.getClass().getSimpleName() + carried);
        }

        ArrayList<String> blobs = new ArrayList<>();
        for (Blob blob : level.blobs.values()) {
            String suffix = "";
            if (blob instanceof SacrificialFire) {
                Item prize = (Item)SACRIFICE_PRIZE.get(blob);
                suffix = ":" + item(prize);
            }
            blobs.add(blob.getClass().getSimpleName() + ":" + blob.volume + suffix);
        }
        blobs.sort(String::compareTo);

        ArrayList<String> custom = new ArrayList<>();
        for (CustomTilemap tile : level.customTiles) {
            custom.add(tile.getClass().getSimpleName() + "@" + tile.tileX + "," + tile.tileY);
        }
        custom.sort(String::compareTo);

        System.out.printf(
                "%s depth=%d seed=%d map=%d door=%s heaps=[%s] mobs=[%s] spawn=[%s] blobs=[%s] custom=[%s] next=%d%n",
                roomClass.getSimpleName(), depth, outerSeed, Arrays.hashCode(level.map), door.type,
                String.join(";", heaps), String.join(";", mobs), items(level.spawnItems()),
                String.join(";", blobs), String.join(";", custom), Random.Long());
        Random.popGenerator();
    }

    public static void main(String[] args) throws Exception {
        long dungeonSeed = args.length > 0 ? Long.parseLong(args[0]) : 0L;
        long outerSeed = args.length > 1 ? Long.parseLong(args[1]) : 0L;
        int depth = args.length > 2 ? Integer.parseInt(args[2]) : 11;
        List<Class<? extends SpecialRoom>> rooms = Arrays.asList(
                WeakFloorRoom.class, CryptRoom.class, PoolRoom.class, ArmoryRoom.class,
                SentryRoom.class, StatueRoom.class, CrystalVaultRoom.class,
                CrystalChoiceRoom.class, SacrificeRoom.class);
        for (Class<? extends SpecialRoom> room : rooms) {
            printCase(room, depth, dungeonSeed, outerSeed);
        }
    }
}
