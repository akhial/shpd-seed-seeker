/*
 * Fixture generator for the Rust RNG parity suite.
 *
 * This intentionally depends only on the JDK. Its helper methods are direct,
 * minimal translations of com.watabou.utils.Random from Shattered Pixel
 * Dungeon v3.3.8 (commit 7b8b845a76fe76c6b7c031ae9e570852411f56db).
 */
import java.util.Arrays;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Random;

public final class RngOracle {
    private static long scrambleSeed(long seed) {
        seed ^= seed >>> 32;
        seed *= 0xbea225f9eb34556dL;
        seed ^= seed >>> 29;
        seed *= 0xbea225f9eb34556dL;
        seed ^= seed >>> 32;
        seed *= 0xbea225f9eb34556dL;
        seed ^= seed >>> 29;
        return seed;
    }

    private static long seedForDepth(long dungeonSeed, int depth, int branch) {
        int lookAhead = depth + 30 * branch;
        Random random = new Random(scrambleSeed(dungeonSeed));
        for (int i = 0; i < lookAhead; i++) {
            random.nextLong();
        }
        return random.nextLong();
    }

    private static int chance(Random random, float[] weights) {
        float sum = 0;
        for (float weight : weights) {
            sum += Math.max(0, weight);
        }
        float value = random.nextFloat() * sum;
        sum = 0;
        for (int i = 0; i < weights.length; i++) {
            sum += Math.max(0, weights[i]);
            if (value < sum) {
                return i;
            }
        }
        return -1;
    }

    public static void main(String[] args) {
        Random basic = new Random(0);
        System.out.printf(
                "basic=%d,%d,%d,%d%n",
                basic.nextInt(),
                basic.nextInt(),
                basic.nextLong(),
                Float.floatToRawIntBits(basic.nextFloat()));

        int[] bounds = {1, 2, 3, 7, 16, 31, 100, 1_073_741_825};
        int[] bounded = new int[bounds.length];
        Random boundRandom = new Random(0x123456789abcdef0L);
        for (int i = 0; i < bounds.length; i++) {
            bounded[i] = boundRandom.nextInt(bounds[i]);
        }
        System.out.println("bounded=" + Arrays.toString(bounded));

        System.out.printf(
                "scramble=%d,%d,%d%n",
                scrambleSeed(0),
                scrambleSeed(1),
                scrambleSeed(-1));

        System.out.printf(
                "depth=%d,%d,%d,%d%n",
                seedForDepth(0, 1, 0),
                seedForDepth(1, 1, 0),
                seedForDepth(1, 5, 0),
                seedForDepth(1, 1, 1));

        Random chanceRandom = new Random(scrambleSeed(1234));
        int[] chances = new int[8];
        for (int i = 0; i < chances.length; i++) {
            chances[i] = chance(chanceRandom, new float[] {1, 2, 3});
        }
        System.out.println("chances=" + Arrays.toString(chances));

        ArrayList<Integer> list = new ArrayList<>();
        for (int i = 0; i < 10; i++) list.add(i);
        Collections.shuffle(list, new Random(scrambleSeed(42)));
        System.out.println("listShuffle=" + list);

        int[] array = {0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
        Random arrayRandom = new Random(scrambleSeed(42));
        for (int i = 0; i < array.length - 1; i++) {
            int j = i + arrayRandom.nextInt(array.length - i);
            int swap = array[i];
            array[i] = array[j];
            array[j] = swap;
        }
        System.out.println("arrayShuffle=" + Arrays.toString(array));
    }
}
