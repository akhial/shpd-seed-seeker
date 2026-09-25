using Xunit;

namespace SeedSeeker.Tests;

public sealed class DailyRunTests
{
    [Fact]
    public void DailyRunsPreserveTheirDateAndRawSeedThroughTheNativeEngine()
    {
        const string date = "2026-09-25";
        Assert.Equal(7_219_798_078_976UL, SeedCode.TryParse(date)!.Value.Value);
        Assert.True(SeedCode.IsScoutable(date));
        Assert.False(SeedCode.IsCanonical(date));
        Assert.False(SeedCode.IsScoutable("2026-02-29"));
        var world = new NativeEngine().Scout(date, 0);
        Assert.Equal(date, world.Seed);
        Assert.NotEmpty(world.Items);
        Assert.Equal("ODAL", world.ItemMappings!.Scrolls[0].Appearance);
    }
}
