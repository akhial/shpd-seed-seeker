package com.shatteredpixel.shatteredpixeldungeon;

import com.shatteredpixel.shatteredpixeldungeon.items.Generator;
import com.shatteredpixel.shatteredpixeldungeon.items.Item;
import com.shatteredpixel.shatteredpixeldungeon.items.artifacts.Artifact;
import com.shatteredpixel.shatteredpixeldungeon.items.scrolls.ScrollOfTransmutation;
import com.watabou.utils.Bundle;

/** Remaining deck and actual transmutations after a canonical floor prefix.
 * Usage: ArtifactOracle AAA-AAA-AAA 9
 */
public final class ArtifactOracle {
    public static void main(String[] args) throws Exception {
        ParityOracle.main(new String[] {args[0], args[1]});
        Bundle state = new Bundle();
        Generator.storeInBundle(state);
        int step = 0;
        Artifact next;
        while ((next = Generator.randomArtifact()) != null) {
            System.out.println("artifact_order " + (++step) + " " + next.getClass().getSimpleName() + " " + next.image);
        }
        Generator.restoreFromBundle(state);
        if (args.length > 2) {
            Item donor = (Item) Class.forName("com.shatteredpixel.shatteredpixeldungeon.items.artifacts." + args[2]).getDeclaredConstructor().newInstance();
            donor.cursed = true;
            ((Artifact) donor).transferUpgrade(5);
            donor.levelKnown = true;
            for (int index = 1; index <= step; index++) {
                donor = ScrollOfTransmutation.changeItem(donor);
                System.out.println("artifact_transmutation " + index + " " + donor.getClass().getSimpleName() + " " + donor.visiblyUpgraded() + " " + donor.cursed);
            }
            System.out.println("artifact_exhausted " + ScrollOfTransmutation.changeItem(donor).getClass().getSimpleName());
        }
    }
}
