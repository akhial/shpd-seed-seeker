/*
 * JDK-only fixtures for the v3.3.8 Caves MobSpawner and constructor order.
 *
 * The tables, Shaman subtype draws, rare checks, DM-200 initializer draw,
 * champion draw, and shuffle are copied statement-for-statement from official
 * commit 7b8b845a76fe76c6b7c031ae9e570852411f56db. `java.util.Random` is the
 * primitive used by watabou Random after its MX3 seed scramble.
 */
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Random;

public final class CavesMobsOracle {
    private enum Mob {
        Bat, Brute, RedShaman, BlueShaman, PurpleShaman, Spinner, DM200,
        ArmoredBrute, DM201, Ghoul
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

    private static Mob shaman(Random random) {
        float roll = random.nextFloat();
        if (roll < 0.4f) return Mob.RedShaman;
        if (roll < 0.8f) return Mob.BlueShaman;
        return Mob.PurpleShaman;
    }

    private static List<Mob> rotation(int depth, Random random) {
        ArrayList<Mob> result;
        switch (depth) {
            case 11:
                result = new ArrayList<>(Arrays.asList(
                        Mob.Bat, Mob.Bat, Mob.Bat, Mob.Brute, shaman(random)));
                break;
            case 12:
                result = new ArrayList<>(Arrays.asList(
                        Mob.Bat, Mob.Bat, Mob.Brute, Mob.Brute,
                        shaman(random), Mob.Spinner));
                break;
            case 13:
                result = new ArrayList<>(Arrays.asList(
                        Mob.Bat, Mob.Brute, Mob.Brute,
                        shaman(random), shaman(random),
                        Mob.Spinner, Mob.Spinner, Mob.DM200));
                break;
            case 14:
                result = new ArrayList<>(Arrays.asList(
                        Mob.Bat, Mob.Brute,
                        shaman(random), shaman(random),
                        Mob.Spinner, Mob.Spinner, Mob.DM200, Mob.DM200));
                if (random.nextFloat() < 0.025f) result.add(Mob.Ghoul);
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
            case Brute: return Mob.ArmoredBrute;
            case DM200: return Mob.DM201;
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
            String loot = "None";
            if (mob == Mob.DM200 || mob == Mob.DM201) {
                loot = random.nextInt(2) == 0 ? "Weapon" : "Armor";
            }
            int champion = random.nextInt(6);
            if (i > 0) output.append(',');
            output.append(mob).append(':').append(champion).append(':').append(loot);
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
        run(11, 0, 10);
        run(13, 1, 16);
        long armoredSeed = findSeed(11, Mob.ArmoredBrute);
        long dm201Seed = findSeed(13, Mob.DM201);
        long ghoulSeed = findSeed(14, Mob.Ghoul);
        run(11, armoredSeed, 5);
        run(13, dm201Seed, 8);
        run(14, ghoulSeed, 9);
    }
}
