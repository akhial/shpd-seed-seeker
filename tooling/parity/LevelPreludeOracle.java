/* Generates mandatory-drop/feeling fixtures through the actual v3.3.8 game. */
import com.shatteredpixel.shatteredpixeldungeon.Dungeon;
import com.watabou.utils.Random;

public final class LevelPreludeOracle {
    public static void main(String[] args) {
        Dungeon.seed = 0;
        Dungeon.LimitedDrops.reset();
        for (int depth = 1; depth <= 4; depth++) {
            Dungeon.depth = depth;
            Random.resetGenerators();
            Random.pushGenerator(Dungeon.seedCurDepth());
            boolean strength = Dungeon.posNeeded();
            if (strength) Dungeon.LimitedDrops.STRENGTH_POTIONS.count++;
            boolean upgrade = Dungeon.souNeeded();
            if (upgrade) Dungeon.LimitedDrops.UPGRADE_SCROLLS.count++;
            boolean stylus = Dungeon.asNeeded();
            if (stylus) Dungeon.LimitedDrops.ARCANE_STYLI.count++;
            boolean enchantStone = Dungeon.enchStoneNeeded();
            if (enchantStone) Dungeon.LimitedDrops.ENCH_STONE.drop();
            boolean intuitionStone = Dungeon.intStoneNeeded();
            if (intuitionStone) Dungeon.LimitedDrops.INT_STONE.drop();
            boolean catalyst = Dungeon.trinketCataNeeded();
            if (catalyst) Dungeon.LimitedDrops.TRINKET_CATA.drop();

            int feeling = -1;
            if (depth > 1) {
                feeling = Random.Int(14);
                if (feeling >= 7) {
                    Random.Float();
                    Random.Float();
                }
            }
            System.out.printf(
                    "d=%d drops=%s,%s,%s,%s,%s,%s feeling=%d next=%d counts=%d,%d,%d%n",
                    depth, strength, upgrade, stylus, enchantStone, intuitionStone, catalyst,
                    feeling, Random.Long(),
                    Dungeon.LimitedDrops.STRENGTH_POTIONS.count,
                    Dungeon.LimitedDrops.UPGRADE_SCROLLS.count,
                    Dungeon.LimitedDrops.ARCANE_STYLI.count);
            Random.popGenerator();
        }
    }
}
