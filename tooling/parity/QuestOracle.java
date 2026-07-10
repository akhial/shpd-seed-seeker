/*
 * Generates quest-reward fixtures through the actual v3.3.8 item classes.
 *
 * Compile this against the pinned desktop jar. The helper deliberately mirrors
 * only the reward statements from the four NPC Quest classes; item, deck,
 * enchantment, glyph, and RNG behavior comes from the game itself. The Imp's
 * ring statements are expanded inline because several ring constructors load
 * textures and cannot be instantiated by this intentionally headless helper.
 */
import com.shatteredpixel.shatteredpixeldungeon.items.Generator;
import com.shatteredpixel.shatteredpixeldungeon.items.Item;
import com.shatteredpixel.shatteredpixeldungeon.items.armor.Armor;
import com.shatteredpixel.shatteredpixeldungeon.items.armor.LeatherArmor;
import com.shatteredpixel.shatteredpixeldungeon.items.armor.MailArmor;
import com.shatteredpixel.shatteredpixeldungeon.items.armor.PlateArmor;
import com.shatteredpixel.shatteredpixeldungeon.items.armor.ScaleArmor;
import com.shatteredpixel.shatteredpixeldungeon.items.potions.Potion;
import com.shatteredpixel.shatteredpixeldungeon.items.rings.Ring;
import com.shatteredpixel.shatteredpixeldungeon.items.scrolls.Scroll;
import com.shatteredpixel.shatteredpixeldungeon.items.weapon.Weapon;
import com.shatteredpixel.shatteredpixeldungeon.items.wands.Wand;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.SpecialRoom;
import com.watabou.utils.Random;

import java.util.ArrayList;

public final class QuestOracle {
    private static void initGenerator(long dungeonSeed) {
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

    private static String effect(Item item) {
        if (item instanceof Weapon && ((Weapon)item).enchantment != null) {
            return ((Weapon)item).enchantment.getClass().getSimpleName();
        }
        if (item instanceof Armor && ((Armor)item).glyph != null) {
            return ((Armor)item).glyph.getClass().getSimpleName();
        }
        return "none";
    }

    private static void printItem(String label, Item item) {
        System.out.printf(
                "%s=%s:+%d:%s:%s ",
                label,
                item.getClass().getSimpleName(),
                item.level(),
                item.cursed ? "cursed" : "clean",
                effect(item));
    }

    private static void ghost(long dungeonSeed, long outerSeed) {
        initGenerator(dungeonSeed);
        Random.pushGenerator(outerSeed);

        Armor armor;
        switch (Random.chances(new float[]{0, 0, 10, 6, 3, 1})) {
            default:
            case 2: armor = new LeatherArmor(); break;
            case 3: armor = new MailArmor(); break;
            case 4: armor = new ScaleArmor(); break;
            case 5: armor = new PlateArmor(); break;
        }
        int tier = Random.chances(new float[]{0, 0, 10, 6, 3, 1});
        Weapon weapon = (Weapon)Generator.random(Generator.wepTiers[tier - 1]);
        weapon.level(0);
        weapon.enchant(null);
        weapon.cursed = false;

        float levelRoll = Random.Float();
        int level = levelRoll < 0.5f ? 0 : levelRoll < 0.8f ? 1 : levelRoll < 0.95f ? 2 : 3;
        weapon.upgrade(level);
        armor.upgrade(level);

        Weapon.Enchantment enchant = Weapon.Enchantment.random();
        Armor.Glyph glyph = Armor.Glyph.random();
        if (Random.Float() <= 0.2f) {
            weapon.enchant(enchant);
            armor.inscribe(glyph);
        }

        System.out.print("ghost ");
        printItem("weapon", weapon);
        printItem("armor", armor);
        System.out.printf(
                "tier2drop=%d tier3drop=%d tier4drop=%d tier5drop=%d next=%d%n",
                Generator.Category.WEP_T2.dropped,
                Generator.Category.WEP_T3.dropped,
                Generator.Category.WEP_T4.dropped,
                Generator.Category.WEP_T5.dropped,
                Random.Long());
        Random.popGenerator();
    }

    private static void wandmaker(long dungeonSeed, long outerSeed) {
        initGenerator(dungeonSeed);
        Random.pushGenerator(outerSeed);

        Wand first = (Wand)Generator.random(Generator.Category.WAND);
        first.cursed = false;
        first.upgrade();
        Wand second = (Wand)Generator.random(Generator.Category.WAND);
        ArrayList<Item> toUndo = new ArrayList<>();
        while (second.getClass() == first.getClass()) {
            toUndo.add(second);
            second = (Wand)Generator.random(Generator.Category.WAND);
        }
        for (Item item : toUndo) Generator.undoDrop(item);
        second.cursed = false;
        second.upgrade();

        System.out.print("wandmaker ");
        printItem("first", first);
        printItem("second", second);
        System.out.printf("wanddrop=%d duplicates=%d next=%d%n",
                Generator.Category.WAND.dropped, toUndo.size(), Random.Long());
        Random.popGenerator();
    }

    private static void blacksmith(long dungeonSeed, long outerSeed) {
        initGenerator(dungeonSeed);
        Random.pushGenerator(outerSeed);

        ArrayList<Item> rewards = new ArrayList<>();
        rewards.add(Generator.randomWeapon(3, true));
        rewards.add(Generator.randomWeapon(3, true));
        ArrayList<Item> toUndo = new ArrayList<>();
        while (rewards.get(0).getClass() == rewards.get(1).getClass()) {
            toUndo.add(rewards.remove(1));
            rewards.add(Generator.randomWeapon(3, true));
        }
        for (Item item : toUndo) Generator.undoDrop(item);
        rewards.add(Generator.randomMissile(3, true));
        rewards.add(Generator.randomArmor(3));

        float levelRoll = Random.Float();
        int level = levelRoll < 0.3f ? 0 : levelRoll < 0.75f ? 1 : levelRoll < 0.95f ? 2 : 3;
        for (Item item : rewards) {
            item.level(level);
            if (item instanceof Weapon) ((Weapon)item).enchant(null);
            else if (item instanceof Armor) ((Armor)item).inscribe(null);
            item.cursed = false;
        }
        Weapon.Enchantment enchant = Weapon.Enchantment.random();
        Armor.Glyph glyph = Armor.Glyph.random();
        if (Random.Float() <= 0.3f) {
            for (Item item : rewards) {
                if (item instanceof Weapon) ((Weapon)item).enchant(enchant);
                else if (item instanceof Armor) ((Armor)item).inscribe(glyph);
            }
        }

        System.out.print("blacksmith-defaults ");
        printItem("weapon0", rewards.get(0));
        printItem("weapon1", rewards.get(1));
        printItem("missile", rewards.get(2));
        printItem("armor", rewards.get(3));
        System.out.printf("duplicates=%d next=%d%n", toUndo.size(), Random.Long());
        Random.popGenerator();
    }

    private static void imp(long dungeonSeed, long outerSeed) {
        initGenerator(dungeonSeed);
        Random.pushGenerator(outerSeed);

        Generator.Category ring = Generator.Category.RING;
        String rewardClass;
        int rewardLevel;
        boolean rewardCursed;
        int rejected = 0;
        do {
            Random.pushGenerator(ring.seed);
            for (int i = 0; i < ring.dropped; i++) Random.Long();
            int index = Random.chances(ring.probs);
            Random.popGenerator();
            ring.probs[index]--;
            ring.dropped++;

            rewardClass = ring.classes[index].getSimpleName();
            rewardLevel = 0;
            if (Random.Int(3) == 0) {
                rewardLevel++;
                if (Random.Int(5) == 0) rewardLevel++;
            }
            rewardCursed = Random.Float() < 0.3f;
            if (rewardCursed) rejected++;
        } while (rewardCursed);
        rewardLevel += 2;
        // Ring.upgrade() consumes this draw once per level even though its
        // curse-clearing side effect is overwritten immediately afterwards.
        Random.Int(3);
        Random.Int(3);
        rewardCursed = true;

        System.out.printf("imp ring=%s:+%d:%s:none ringdrop=%d rejected=%d next=%d%n",
                rewardClass, rewardLevel, rewardCursed ? "cursed" : "clean",
                ring.dropped, rejected, Random.Long());
        Random.popGenerator();
    }

    public static void main(String[] args) {
        long dungeonSeed = args.length > 0 ? Long.parseLong(args[0]) : 0L;
        long outerSeed = args.length > 1 ? Long.parseLong(args[1]) : 0L;
        ghost(dungeonSeed, outerSeed);
        wandmaker(dungeonSeed, outerSeed);
        blacksmith(dungeonSeed, outerSeed);
        imp(dungeonSeed, outerSeed);
    }
}
