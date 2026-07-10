/* Focused v3.3.8 RegularLevel.randomDropCell spatial/RNG fixtures. */
import com.shatteredpixel.shatteredpixeldungeon.actors.mobs.Mob;
import com.shatteredpixel.shatteredpixeldungeon.levels.Level;
import com.shatteredpixel.shatteredpixeldungeon.levels.RegularLevel;
import com.shatteredpixel.shatteredpixeldungeon.levels.Terrain;
import com.shatteredpixel.shatteredpixeldungeon.levels.painters.Painter;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.Room;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.connection.TunnelRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.special.SpecialRoom;
import com.shatteredpixel.shatteredpixeldungeon.levels.rooms.standard.StandardRoom;
import com.watabou.utils.Point;
import com.watabou.utils.Random;
import com.watabou.utils.Rect;
import com.watabou.utils.SparseArray;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;

public final class RegularPlacementOracle {
    private static final ArrayList<String> TRACE = new ArrayList<>();

    private static final class TaggedStandard extends StandardRoom {
        final String tag;
        int rejects;

        TaggedStandard(String tag, Rect bounds, int rejects) {
            this.tag = tag;
            this.rejects = rejects;
            set(bounds);
        }

        @Override public void paint(Level level) {}

        @Override public Point random() {
            Point point = super.random();
            TRACE.add("point:" + tag + ":" + point.x + "," + point.y);
            return point;
        }

        @Override public boolean canPlaceItem(Point point, Level level) {
            TRACE.add("rule:" + tag + ":" + point.x + "," + point.y);
            if (rejects > 0) {
                rejects--;
                return false;
            }
            return super.canPlaceItem(point, level);
        }
    }

    private static final class TaggedSpecial extends SpecialRoom {
        final String tag;

        TaggedSpecial(String tag, Rect bounds) {
            this.tag = tag;
            set(bounds);
        }

        @Override public void paint(Level level) {}

        @Override public Point random() {
            Point point = super.random();
            TRACE.add("point:" + tag + ":" + point.x + "," + point.y);
            return point;
        }
    }

    private static final class OracleLevel extends RegularLevel {
        final HashMap<Room, String> tags = new HashMap<>();
        int fixedExit = 0;

        OracleLevel(int rejectsA, int rejectsB) {
            setSize(24, 12);
            for (int i = 0; i < map.length; i++) map[i] = Terrain.EMPTY;
            mobs = new HashSet<>();
            heaps = new SparseArray<>();
            plants = new SparseArray<>();
            traps = new SparseArray<>();
            blobs = new HashMap<>();

            TaggedStandard entrance = new TaggedStandard("E", new Rect(1, 1, 5, 5), 0);
            TaggedStandard a = new TaggedStandard("A", new Rect(7, 1, 13, 8), rejectsA);
            TaggedStandard b = new TaggedStandard("B", new Rect(14, 1, 20, 8), rejectsB);
            TaggedSpecial special = new TaggedSpecial("S", new Rect(7, 8, 13, 11));
            TunnelRoom tunnel = new TunnelRoom();
            tunnel.set(new Rect(1, 7, 5, 10));

            rooms = new ArrayList<>();
            rooms.add(entrance);
            rooms.add(a);
            rooms.add(b);
            rooms.add(special);
            rooms.add(tunnel);
            roomEntrance = entrance;
            tags.put(entrance, "E");
            tags.put(a, "A");
            tags.put(b, "B");
            tags.put(special, "S");
            tags.put(tunnel, "T");
            buildFlagMaps();
        }

        @Override protected Painter painter() { return null; }
        @Override protected void createMobs() {}
        @Override protected void createItems() {}
        @Override public int exit() { return fixedExit; }

        @Override protected Room randomRoom(Class<? extends Room> type) {
            Room room = super.randomRoom(type);
            TRACE.add("select:" + (room == null ? "null" : tags.get(room)));
            return room;
        }

        int pickStandard() { return randomDropCell(); }
        int pickSpecial() { return randomDropCell(SpecialRoom.class); }

        String order() {
            StringBuilder result = new StringBuilder();
            for (Room room : rooms) result.append(tags.get(room));
            return result.toString();
        }
    }

    private static void run(long seed, int rejectsA, int rejectsB, boolean special) {
        TRACE.clear();
        Random.resetGenerators();
        OracleLevel level = new OracleLevel(rejectsA, rejectsB);
        Random.pushGenerator(seed);
        int cell = special ? level.pickSpecial() : level.pickStandard();
        long next = Random.Long();
        Random.popGenerator();
        Point point = cell < 0 ? new Point(-1, -1) : level.cellToPoint(cell);
        System.out.printf(
                "seed=%d rejects=%d,%d kind=%s cell=%d point=%d,%d order=%s next=%d%n",
                seed, rejectsA, rejectsB, special ? "special" : "standard",
                cell, point.x, point.y, level.order(), next);
        for (String event : TRACE) System.out.println(event);
    }

    public static void main(String[] args) {
        if (args.length >= 1) {
            long seed = Long.parseLong(args[0]);
            int rejectsA = args.length >= 2 ? Integer.parseInt(args[1]) : 0;
            int rejectsB = args.length >= 3 ? Integer.parseInt(args[2]) : 0;
            boolean special = args.length >= 4 && Boolean.parseBoolean(args[3]);
            run(seed, rejectsA, rejectsB, special);
            return;
        }
        run(123, 0, 0, false);
        run(123, 2, 2, false);
        run(4, 0, 0, false);
        run(9876, 0, 0, true);
    }
}
