/* Generates isolated ShopRoom fixtures through the actual v3.3.8 classes. */
import com.shatteredpixel.shatteredpixeldungeon.Dungeon;
import com.shatteredpixel.shatteredpixeldungeon.actors.hero.Hero;
import com.shatteredpixel.shatteredpixeldungeon.items.Generator;
import com.shatteredpixel.shatteredpixeldungeon.items.Item;
import com.shatteredpixel.shatteredpixeldungeon.items.armor.Armor;
import com.shatteredpixel.shatteredpixeldungeon.items.bags.Bag;
import com.shatteredpixel.shatteredpixeldungeon.items.potions.Potion;
import com.shatteredpixel.shatteredpixeldungeon.items.rings.Ring;
import com.shatteredpixel.shatteredpixeldungeon.items.scrolls.Scroll;
import com.shatteredpixel.shatteredpixeldungeon.items.weapon.Weapon;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.ShopRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.SpecialRoom;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.backends.lwjgl3.Lwjgl3Files;
import com.badlogic.gdx.utils.GdxNativesLoader;
import com.watabou.utils.Random;

import java.lang.reflect.Method;
import java.util.ArrayList;

public final class ShopOracle {
    private static final Method GENERATE;

    static {
        try {
            GENERATE = ShopRoom.class.getDeclaredMethod("generateItems");
            GENERATE.setAccessible(true);
        } catch (ReflectiveOperationException error) {
            throw new ExceptionInInitializerError(error);
        }
    }

    private static void initRun(long dungeonSeed) {
        GdxNativesLoader.load();
        Gdx.files = new Lwjgl3Files();
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

        Dungeon.LimitedDrops.reset();
        // The canonical Warrior starts with the pouch already collected.
        Dungeon.LimitedDrops.VELVET_POUCH.drop();
        Dungeon.hero = new Hero();
    }

    @SuppressWarnings("unchecked")
    private static ArrayList<Item> generate(int depth, long outerSeed) throws Exception {
        Dungeon.depth = depth;
        Random.pushGenerator(outerSeed);
        ArrayList<Item> result = (ArrayList<Item>)GENERATE.invoke(null);
        System.out.printf("depth=%d seed=%d next=%d size=%d%n",
                depth, outerSeed, Random.Long(), result.size());
        Random.popGenerator();
        return result;
    }

    private static String describe(Item item) {
        // Bag identity is intentionally normalized: ChooseBag uses a HashMap
        // whose identity-hash iteration order is not a seeded-world property.
        String type = item instanceof Bag ? "Bag" : item.getClass().getSimpleName();
        String effect = "none";
        if (item instanceof Weapon && ((Weapon)item).enchantment != null) {
            effect = ((Weapon)item).enchantment.getClass().getSimpleName();
        } else if (item instanceof Armor && ((Armor)item).glyph != null) {
            effect = ((Armor)item).glyph.getClass().getSimpleName();
        }
        return String.format("%s:q%d:l%d:c%s:e%s",
                type, item.quantity(), item.level(), item.cursed, effect);
    }

    public static void main(String[] args) throws Exception {
        long dungeonSeed = args.length > 0 ? Long.parseLong(args[0]) : 0L;
        long outerSeed = args.length > 1 ? Long.parseLong(args[1]) : 0L;
        initRun(dungeonSeed);
        for (int depth : new int[]{6, 11, 16, 20}) {
            ArrayList<Item> items = generate(depth, outerSeed + depth);
            for (int index = 0; index < items.size(); index++) {
                System.out.printf("%02d %s%n", index, describe(items.get(index)));
            }
        }
    }
}
