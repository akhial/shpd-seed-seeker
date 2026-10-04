/* Shattered Pixel Dungeon vault door selectors. GPL-3.0-or-later. */
package com.shatteredpixel.shatteredpixeldungeon;

import com.badlogic.gdx.utils.JsonReader;
import com.badlogic.gdx.utils.JsonValue;
import com.shatteredpixel.shatteredpixeldungeon.levels.SewerLevel;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.quest.vault.VaultFinalRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.quest.vault.VaultTokensRoom;
import com.shatteredpixel.shatteredpixeldungeon.tiles.CustomTilemap;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

/** Evaluates the original custom tile selectors without allocating GL textures. */
public final class VaultDoorOracle {
    private static int[] tiles(CustomTilemap tilemap, int x, int y, int height) throws Exception {
        tilemap.pos(x, y);
        int[] data = new int[height];
        Method update = tilemap.getClass().getDeclaredMethod("updateAll", int[].class);
        update.setAccessible(true);
        update.invoke(tilemap, (Object)data);
        return data;
    }

    public static void main(String[] args) throws Exception {
        com.watabou.noosa.Game.version = "4.0.1";
        JsonValue doc = new JsonReader().parse(Files.readString(Path.of(args[0])));
        Dungeon.level = new SewerLevel();
        int width = doc.getInt("width");
        Dungeon.level.setSize(width, doc.getInt("height"));
        Dungeon.level.map = doc.get("terrain").asIntArray();
        StringBuilder result = new StringBuilder("{\"seed\":\"")
            .append(doc.getString("seed")).append("\",\"depth\":")
            .append(doc.getInt("depth")).append(",\"doors\":[");
        boolean first = true;
        for (JsonValue feature : doc.get("contents").get("features")) {
            String kind = feature.getString("kind");
            int cell = feature.getInt("cell"), x = cell % width, y = cell / width;
            int[] floor, walls;
            if (kind.equals("VaultTokenDoorFloor")) {
                floor = tiles(new VaultTokensRoom.TokenDoorFloor(), x, y, 1);
                walls = new int[]{-1, -1};
            } else if (kind.equals("VaultFinalDoor")) {
                floor = tiles(new VaultFinalRoom.FinalRoomDoor(), x, y, 1);
                walls = tiles(new VaultFinalRoom.FinalRoomDoorOverhang(), x, y-1, 2);
            } else continue;
            if (!first) result.append(',');
            first = false;
            result.append("{\"cell\":").append(cell).append(",\"kind\":\"").append(kind)
                .append("\",\"floor\":").append(floor[0])
                .append(",\"walls\":").append(Arrays.toString(walls)).append('}');
        }
        System.out.println(result.append("]}"));
    }
}
