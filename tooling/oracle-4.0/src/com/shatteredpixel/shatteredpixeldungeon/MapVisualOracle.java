/* Shattered Pixel Dungeon raised tile selection oracle. GPL-3.0-or-later. */
package com.shatteredpixel.shatteredpixeldungeon;

import com.badlogic.gdx.utils.JsonReader;
import com.badlogic.gdx.utils.JsonValue;
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.npcs.Blacksmith;
import com.shatteredpixel.shatteredpixeldungeon.levels.*;
import com.shatteredpixel.shatteredpixeldungeon.tiles.*;
import com.shatteredpixel.shatteredpixeldungeon.utils.DungeonSeed;
import com.watabou.noosa.Tilemap;
import com.watabou.utils.SparseArray;
import java.nio.file.*;
import java.lang.reflect.*;
import java.util.Arrays;
import sun.misc.Unsafe;

/** Runs the unmodified game tile selectors without allocating GL textures.
 * Input is a level-map JSON document. Output hashes every cell of each selected
 * atlas layer before any player-dependent fog, entities or custom tilemaps.
 * The rendering classes come from the original JAR; allocation alone is bypassed.
 */
public final class MapVisualOracle {
    private static void set(Object target, Class<?> owner, String name, Object value) throws Exception {
        Field field=owner.getDeclaredField(name);field.setAccessible(true);field.set(target,value);
    }
    public static void main(String[] args) throws Exception {
        Field field=Unsafe.class.getDeclaredField("theUnsafe");field.setAccessible(true);
        Unsafe unsafe=(Unsafe)field.get(null);
        JsonValue doc=new JsonReader().parse(Files.readString(Path.of(args[0])));
        com.watabou.noosa.Game.version="4.0.0";
        String kind=doc.getString("kind");
        Dungeon.seed=DungeonSeed.convertFromCode(doc.getString("seed"));
        Dungeon.depth=doc.getInt("depth");Dungeon.branch=doc.getInt("branch");
        set(null,Blacksmith.Quest.class,"type",kind.equals("blacksmith_crystal")?1:2);
        Level level=kind.startsWith("blacksmith_")?new MiningLevel():new SewerLevel();
        Dungeon.level=level;
        level.setSize(doc.getInt("width"),doc.getInt("height"));
        level.map=doc.get("terrain").asIntArray();
        for(int i=0;i<level.map.length;i++)level.pit[i]=level.map[i]==Terrain.CHASM;
        level.cleanWalls();
        DungeonTileSheet.setupVariance(level.map.length,Dungeon.seedCurDepth());
        StringBuilder result=new StringBuilder("{\"seed\":\""+doc.getString("seed")+"\",\"depth\":"+Dungeon.depth+",\"branch\":"+Dungeon.branch+",\"kind\":\""+kind+"\",\"terrainHash\":"+Arrays.hashCode(level.map)+",\"layers\":{");
        Class<?>[] classes={DungeonTerrainTilemap.class,WallOcclusionTilemap.class,TerrainFeaturesTilemap.class,RaisedTerrainTilemap.class,DungeonWallsTilemap.class};
        String[] names={"terrain","shadows","features","raised","walls"};
        for(int n=0;n<classes.length;n++){
            Object tilemap=unsafe.allocateInstance(classes[n]);
            set(tilemap,DungeonTilemap.class,"map",level.map);
            set(tilemap,Tilemap.class,"mapWidth",level.width());
            set(tilemap,Tilemap.class,"size",level.length());
            if(tilemap instanceof TerrainFeaturesTilemap){
                set(tilemap,TerrainFeaturesTilemap.class,"plants",new SparseArray<>());
                set(tilemap,TerrainFeaturesTilemap.class,"traps",new SparseArray<>());
            }
            Method method=classes[n].getDeclaredMethod("getTileVisual",int.class,int.class,boolean.class);method.setAccessible(true);
            int[] visuals=new int[level.length()];
            for(int i=0;i<visuals.length;i++){
                visuals[i]=(Integer)method.invoke(tilemap,i,level.map[i],false);
                if(n==1 && visuals[i]==0)visuals[i]=-1;
            }
            if(n>0)result.append(',');
            result.append('"').append(names[n]).append("\":").append(Arrays.hashCode(visuals));
            if(args.length>1)Files.writeString(Path.of(args[1]+"-"+names[n]+".json"),Arrays.toString(visuals));
        }
        System.out.println(result.append("}}"));
    }
}
