/*
 * JDK-only fixtures for the v3.3.8 Prison MobSpawner and constructor order.
 *
 * The tables, rare checks, initializer draws, champion draw, and shuffle are
 * copied statement-for-statement from official commit
 * 7b8b845a76fe76c6b7c031ae9e570852411f56db. `java.util.Random` is the
 * primitive used by watabou Random after its MX3 seed scramble. Full-floor
 * class/cell and Wandmaker generator checkpoints are additionally captured
 * by `tooling/oracle/run.sh --seed AAA-AAA-AAA --floors 6-9`.
 */
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Random;

public final class PrisonMobsOracle {
    private enum Mob {
        Skeleton, Thief, Swarm, DM100, Guard, Necromancer,
        Bandit, SpectralNecromancer, Bat
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
            case 6:
                result = new ArrayList<>(Arrays.asList(
                        Mob.Skeleton, Mob.Skeleton, Mob.Skeleton, Mob.Thief, Mob.Swarm));
                break;
            case 7:
                result = new ArrayList<>(Arrays.asList(
                        Mob.Skeleton, Mob.Skeleton, Mob.Skeleton, Mob.Thief,
                        Mob.DM100, Mob.Guard));
                break;
            case 8:
                result = new ArrayList<>(Arrays.asList(
                        Mob.Skeleton, Mob.Skeleton, Mob.Thief,
                        Mob.DM100, Mob.DM100, Mob.Guard, Mob.Guard, Mob.Necromancer));
                break;
            case 9:
                result = new ArrayList<>(Arrays.asList(
                        Mob.Skeleton, Mob.Thief, Mob.DM100, Mob.DM100,
                        Mob.Guard, Mob.Guard, Mob.Necromancer, Mob.Necromancer));
                if (random.nextFloat() < 0.025f) result.add(Mob.Bat);
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
            case Thief: return Mob.Bandit;
            case Necromancer: return Mob.SpectralNecromancer;
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
            if (mob == Mob.Thief || mob == Mob.Bandit) {
                thiefLoot = random.nextInt(2) == 0 ? "Ring" : "Artifact";
            }
            int champion = random.nextInt(6);
            if (i > 0) output.append(',');
            output.append(mob).append(':').append(champion).append(':').append(thiefLoot);
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
        run(6, 0, 10);
        run(8, 1, 16);
        long banditSeed = findSeed(6, Mob.Bandit);
        long spectralSeed = findSeed(9, Mob.SpectralNecromancer);
        long batSeed = findSeed(9, Mob.Bat);
        run(6, banditSeed, 5);
        run(9, spectralSeed, 9);
        run(9, batSeed, 9);
    }
}
