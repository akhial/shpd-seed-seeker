/*
 * JDK-only fixtures for the v3.3.8 Halls MobSpawner and constructor order.
 *
 * The tables, no-op rare insertion boundary, per-entry alternate checks,
 * champion draw, and Java shuffle are copied statement-for-statement from
 * official commit 7b8b845a76fe76c6b7c031ae9e570852411f56db.
 */
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Random;

public final class HallsMobsOracle {
    private enum Mob { Succubus, Eye, Scorpio, Acidic }

    private static long scramble(long seed) {
        seed ^= seed >>> 32;
        seed *= 0xbea225f9eb34556dL;
        seed ^= seed >>> 29;
        seed *= 0xbea225f9eb34556dL;
        seed ^= seed >>> 32;
        seed *= 0xbea225f9eb34556dL;
        seed ^= seed >>> 29;
        return seed;
    }

    private static List<Mob> rotation(int depth, Random random) {
        ArrayList<Mob> result;
        switch (depth) {
            case 21:
                result = new ArrayList<>(Arrays.asList(Mob.Succubus, Mob.Succubus, Mob.Eye));
                break;
            case 22:
                result = new ArrayList<>(Arrays.asList(Mob.Succubus, Mob.Eye));
                break;
            case 23:
                result = new ArrayList<>(Arrays.asList(Mob.Succubus, Mob.Eye, Mob.Eye, Mob.Scorpio));
                break;
            case 24:
                result = new ArrayList<>(Arrays.asList(
                        Mob.Succubus, Mob.Eye, Mob.Eye,
                        Mob.Scorpio, Mob.Scorpio, Mob.Scorpio));
                break;
            default:
                throw new IllegalArgumentException("depth");
        }
        // addRareMobs returns without consuming RNG for every Halls depth.
        for (int i = 0; i < result.size(); i++) {
            if (random.nextFloat() < 1 / 50f && result.get(i) == Mob.Scorpio) {
                result.set(i, Mob.Acidic);
            }
        }
        Collections.shuffle(result, random);
        return result;
    }

    private static void run(int depth, long seed, int count) {
        Random random = new Random(scramble(seed));
        ArrayList<Mob> queue = new ArrayList<>();
        StringBuilder output = new StringBuilder();
        for (int i = 0; i < count; i++) {
            if (queue.isEmpty()) queue.addAll(rotation(depth, random));
            Mob mob = queue.remove(0);
            int champion = random.nextInt(6);
            if (i > 0) output.append(',');
            output.append(mob).append(':').append(champion);
        }
        System.out.printf("depth=%d seed=%d mobs=%s next=%d%n",
                depth, seed, output, random.nextLong());
    }

    private static long findSeed(int depth, Mob target) {
        for (long seed = 0; seed < 1_000_000; seed++) {
            if (rotation(depth, new Random(scramble(seed))).contains(target)) return seed;
        }
        throw new AssertionError("coverage seed not found for " + target);
    }

    public static void main(String[] args) {
        run(21, 0, 9);
        run(22, 2, 8);
        run(23, 1, 12);
        long acidicSeed = findSeed(24, Mob.Acidic);
        run(24, acidicSeed, 6);
    }
}
