/*
 * JDK-only fixture generator for v3.3.8 MobSpawner's Sewer rotations.
 *
 * The tables and call order are copied from official commit
 * 7b8b845a76fe76c6b7c031ae9e570852411f56db. java.util.Random and
 * Collections.shuffle are the actual primitives used by watabou Random.
 */
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Random;

public final class MobRotationOracle {
    private enum Mob {
        Rat, Snake, Gnoll, Swarm, Crab, Slime,
        Albino, GnollExile, HermitCrab, CausticSlime, Thief
    }

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
            case 1:
                result = new ArrayList<>(Arrays.asList(Mob.Rat, Mob.Rat, Mob.Rat, Mob.Snake));
                break;
            case 2:
                result = new ArrayList<>(Arrays.asList(
                        Mob.Rat, Mob.Rat, Mob.Snake, Mob.Gnoll, Mob.Gnoll));
                break;
            case 3:
                result = new ArrayList<>(Arrays.asList(
                        Mob.Rat, Mob.Snake, Mob.Gnoll, Mob.Gnoll, Mob.Gnoll, Mob.Swarm, Mob.Crab));
                break;
            case 4:
                result = new ArrayList<>(Arrays.asList(
                        Mob.Gnoll, Mob.Swarm, Mob.Crab, Mob.Crab, Mob.Slime, Mob.Slime));
                if (random.nextFloat() < 0.025f) result.add(Mob.Thief);
                break;
            default:
                throw new IllegalArgumentException("depth");
        }
        for (int i = 0; i < result.size(); i++) {
            if (random.nextFloat() < 1 / 50f) result.set(i, alt(result.get(i)));
        }
        Collections.shuffle(result, random);
        return result;
    }

    private static Mob alt(Mob mob) {
        switch (mob) {
            case Rat: return Mob.Albino;
            case Gnoll: return Mob.GnollExile;
            case Crab: return Mob.HermitCrab;
            case Slime: return Mob.CausticSlime;
            default: return mob;
        }
    }

    private static void run(int depth, long seed, int count) {
        Random random = new Random(scramble(seed));
        ArrayList<Mob> queue = new ArrayList<>();
        StringBuilder output = new StringBuilder();
        for (int i = 0; i < count; i++) {
            if (queue.isEmpty()) queue.addAll(rotation(depth, random));
            Mob mob = queue.remove(0);
            String thiefLoot = "None";
            if (mob == Mob.Thief) thiefLoot = random.nextInt(2) == 0 ? "Ring" : "Artifact";
            int champion = random.nextInt(6);
            if (i > 0) output.append(',');
            output.append(mob).append(':').append(champion).append(':').append(thiefLoot);
        }
        System.out.printf("depth=%d seed=%d mobs=%s next=%d%n",
                depth, seed, output, random.nextLong());
    }

    private static long findSeed(int depth, Mob target) {
        for (long seed = 0; seed < 100_000; seed++) {
            if (rotation(depth, new Random(scramble(seed))).contains(target)) return seed;
        }
        throw new AssertionError("coverage seed not found for " + target);
    }

    public static void main(String[] args) {
        run(1, 0, 8);
        run(3, 1, 14);
        long albinoSeed = findSeed(1, Mob.Albino);
        long thiefSeed = findSeed(4, Mob.Thief);
        run(1, albinoSeed, 4);
        run(4, thiefSeed, 7);
    }
}
