package com.shatteredpixel.shatteredpixeldungeon;

import com.shatteredpixel.shatteredpixeldungeon.sprites.ItemSpriteSheet;

/** Export the evaluated film, including range assignments and later overrides. */
public final class ItemFilmOracle {
    public static void main(String[] args) {
        System.out.println("//! Evaluated `ItemSpriteSheet` film from the pinned v4.0.0 JAR.");
        System.out.println("//! Regenerate with tooling/oracle-4.0 `ItemFilmOracle`.");
        System.out.println("pub(super) const ITEM_SIZES: [[u16; 2]; 512] = [");
        for (int i = 0; i < 512; i++) {
            System.out.printf("    [%d, %d],%n", Math.round(ItemSpriteSheet.film.width(i)), Math.round(ItemSpriteSheet.film.height(i)));
        }
        System.out.println("];\n");
    }
}
