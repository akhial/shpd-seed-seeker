package com.shatteredpixel.shatteredpixeldungeon;

import com.shatteredpixel.shatteredpixeldungeon.items.Generator;
import com.shatteredpixel.shatteredpixeldungeon.items.Item;
import com.shatteredpixel.shatteredpixeldungeon.items.scrolls.ScrollOfTransmutation;
import com.watabou.utils.Bundle;

/** Prints the private trinket deck after the normal oracle initializes a run.
 * Floor generation must not consume this deck in the canonical profile.
 * Usage: TrinketOracle AAA-AAA-AAA [maximum-floor]
 */
public final class TrinketOracle {
    public static void main(String[] args) {
        ParityOracle.main(new String[] {args[0], args.length > 1 ? args[1] : "1"});
        Bundle initialDeck = new Bundle();
        Generator.storeInBundle(initialDeck);
        for (int i = 0; i < 17; i++) {
            Item item = Generator.random(Generator.Category.TRINKET);
            System.out.println("trinket_order " + (i + 1) + " "
                    + item.getClass().getSimpleName() + " " + item.image);
        }
        // The catalyst draws all four offers before the player picks one.
        // Exercise the actual shipped transmutation method for every choice.
        for (int choice = 0; choice < 4; choice++) {
            Generator.restoreFromBundle(initialDeck);
            Item chosen = null;
            for (int i = 0; i < 4; i++) {
                Item offer = Generator.random(Generator.Category.TRINKET);
                if (i == choice) chosen = offer;
            }
            for (int step = 1; step <= 13; step++) {
                chosen = ScrollOfTransmutation.changeItem(chosen);
                System.out.println("trinket_transmutation " + choice + " " + step + " "
                        + chosen.getClass().getSimpleName() + " " + chosen.image);
            }
        }
    }
}
