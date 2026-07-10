/* Generates equipment-roll fixtures through the actual v3.3.8 game classes. */
import com.shatteredpixel.shatteredpixeldungeon.items.Item;
import com.shatteredpixel.shatteredpixeldungeon.items.armor.Armor;
import com.shatteredpixel.shatteredpixeldungeon.items.armor.MailArmor;
import com.shatteredpixel.shatteredpixeldungeon.items.wands.WandOfFrost;
import com.shatteredpixel.shatteredpixeldungeon.items.weapon.Weapon;
import com.shatteredpixel.shatteredpixeldungeon.items.weapon.melee.Sword;
import com.watabou.utils.Random;

public final class EquipmentOracle {
    private static void print(Item item) {
        String effect = "none";
        if (item instanceof Weapon && ((Weapon)item).enchantment != null) {
            effect = ((Weapon)item).enchantment.getClass().getSimpleName();
        } else if (item instanceof Armor && ((Armor)item).glyph != null) {
            effect = ((Armor)item).glyph.getClass().getSimpleName();
        }
        System.out.printf(
                "%s level=%d cursed=%s effect=%s%n",
                item.getClass().getSimpleName(), item.level(), item.cursed, effect);
    }

    public static void main(String[] args) {
        Random.resetGenerators();
        Random.pushGenerator(8_687_205_886L);
        print(new Sword().random());
        print(new MailArmor().random());
        print(new WandOfFrost().random());
        print(new Sword().random());
        print(new MailArmor().random());
        print(new WandOfFrost().random());
        Random.popGenerator();

        int weaponEffects = 0;
        int armorEffects = 0;
        for (long seed = 0; seed < 10_000 && (weaponEffects < 4 || armorEffects < 4); seed++) {
            Random.resetGenerators();
            Random.pushGenerator(seed);
            Sword weapon = (Sword)new Sword().random();
            if (weapon.enchantment != null && weaponEffects++ < 4) {
                System.out.print("seed=" + seed + " ");
                print(weapon);
            }
            Random.popGenerator();

            Random.resetGenerators();
            Random.pushGenerator(seed);
            MailArmor armor = (MailArmor)new MailArmor().random();
            if (armor.glyph != null && armorEffects++ < 4) {
                System.out.print("seed=" + seed + " ");
                print(armor);
            }
            Random.popGenerator();
        }
    }
}
