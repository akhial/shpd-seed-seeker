/* Shattered Pixel Dungeon mining-map parity oracle. GPL-3.0-or-later. */
package com.shatteredpixel.shatteredpixeldungeon;

import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.npcs.Blacksmith;
import com.shatteredpixel.shatteredpixeldungeon.levels.Level;
import com.shatteredpixel.shatteredpixeldungeon.levels.RegularLevel;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.Room;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.secret.SecretRoom;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.util.Arrays;

/** Isolates MiningLevel.create() on its branch root, using the unmodified game JAR.
 * Arguments: seed code, depth (12..14), quest type (1 Crystal / 2 Gnoll), challenge mask.
 * The existing headless harness initializes settings, the hero and empty bones state.
 * Quest type is deliberately forced: the map endpoint separately tests quest scheduling.
 */
public final class MiningMapOracle {
    public static void main(String[] args) throws Exception {
        PrintStream stdout = System.out;
        try {
            System.setOut(new PrintStream(OutputStream.nullOutputStream()));
            ParityOracle.main(new String[]{"--seed", args[0], "--floors", "1", "--challenges", args[3]});
        } finally {
            System.setOut(stdout);
        }
        Field type = Blacksmith.Quest.class.getDeclaredField("type");
        type.setAccessible(true);
        type.setInt(null, Integer.parseInt(args[2]));
        Dungeon.depth = Integer.parseInt(args[1]);
        Dungeon.branch = 1;
        Level level = Dungeon.newLevel();
        StringBuilder secrets = new StringBuilder("[");
        for (Room room : ((RegularLevel) level).rooms()) {
            if (room instanceof SecretRoom) {
                if (secrets.length() > 1) secrets.append(',');
                secrets.append('[').append(room.left).append(',').append(room.top).append(',')
                        .append(room.right).append(',').append(room.bottom).append(']');
            }
        }
        secrets.append(']');
        int[] traps = level.traps.keyArray();
        Arrays.sort(traps);
        System.out.println("{\"seed\":\"" + args[0] + "\",\"depth\":" + Dungeon.depth
                + ",\"variant\":" + args[2] + ",\"challenges\":" + args[3]
                + ",\"width\":" + level.width() + ",\"height\":" + level.height()
                + ",\"hash\":" + Arrays.hashCode(level.map) + ",\"entrance\":" + level.entrance()
                + ",\"secretRooms\":" + secrets + ",\"traps\":" + Arrays.toString(traps) + "}");
    }
}
