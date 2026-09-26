using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json.Nodes;

namespace SeedSeeker;

internal static partial class Native
{
    private const string Library = "shpd_seedfinder_ffi";
    [LibraryImport(Library)] internal static partial long seedfinder_start_search(byte[] request, nuint length, uint workers);
    [LibraryImport(Library)] internal static partial long seedfinder_start_resumed_search(byte[] request, nuint length, ulong resumeFrom, ulong scanLength, uint workers);
    [LibraryImport(Library)] internal static partial uint seedfinder_available_workers();
    [LibraryImport(Library)] internal static partial int seedfinder_poll(long handle, uint maximum, out nint packet, out nuint length);
    [LibraryImport(Library)] internal static partial int seedfinder_status(long handle, [Out] long[] status);
    [LibraryImport(Library)] internal static partial int seedfinder_resume_hint(long handle, [Out] long[] hint);
    [LibraryImport(Library)] internal static partial void seedfinder_cancel(long handle);
    [LibraryImport(Library)] internal static partial void seedfinder_close(long handle);
    [LibraryImport(Library)] internal static partial int seedfinder_scout(byte[] request, nuint length, out nint packet, out nuint outputLength);
    [LibraryImport(Library)] internal static partial int seedfinder_level_map(byte[] request, nuint length, out nint packet, out nuint outputLength);
    [LibraryImport(Library)] internal static partial int seedfinder_level_map_asset(byte[] id, nuint length, out nint packet, out nuint outputLength);
    [LibraryImport(Library)] internal static partial int seedfinder_scout_matches(byte[] request, nuint length, byte[] query, nuint queryLength, out nint packet, out nuint outputLength);
    [LibraryImport(Library)] internal static partial int seedfinder_filter_seeds(byte[] request, nuint length, ulong[] seeds, nuint seedsLength, out nint packet, out nuint outputLength);
    [LibraryImport(Library)] internal static partial int seedfinder_seed_format(byte[] input, nuint length, out nint packet, out nuint outputLength);
    [LibraryImport(Library)] internal static partial int seedfinder_seed_parse(byte[] input, nuint length, out nint packet, out nuint outputLength);
    [LibraryImport(Library)] internal static partial int seedfinder_share_encode(byte[] queryJson, nuint length, out nint packet, out nuint outputLength);
    [LibraryImport(Library)] internal static partial int seedfinder_share_decode(byte[] text, nuint length, out nint packet, out nuint outputLength);
    [LibraryImport(Library)] internal static partial int seedfinder_query_impossibility_reason(byte[] text, nuint length, out nint packet, out nuint outputLength);
    [LibraryImport(Library)] internal static partial int seedfinder_results_encode(byte[] request, nuint length, out nint packet, out nuint outputLength);
    [LibraryImport(Library)] internal static partial int seedfinder_results_decode(byte[] contents, nuint length, out nint packet, out nuint outputLength);
    [LibraryImport(Library)] internal static partial int seedfinder_engine_info(out nint packet, out nuint outputLength);
    [LibraryImport(Library)] internal static partial void seedfinder_buffer_free(nint packet, nuint length);
}

internal sealed class Writer
{
    private readonly MemoryStream stream = new();
    public void Bytes(IEnumerable<byte> bytes) { foreach (var b in bytes) stream.WriteByte(b); }
    public void U8(int value) => stream.WriteByte((byte)value);
    public void U16(int value) { U8(value >> 8); U8(value); }
    public void U16Le(int value) { U8(value); U8(value >> 8); }
    public void Text(string value) { var b = Encoding.UTF8.GetBytes(value); U16(b.Length); Bytes(b); }
    public byte[] Finish() => stream.ToArray();
}

internal ref struct Reader
{
    private readonly ReadOnlySpan<byte> data; private int offset;
    public Reader(byte[] bytes) { data = bytes; offset = 0; }
    public int Remaining => data.Length - offset;
    public byte U8() { if (Remaining < 1) throw new InvalidDataException("Truncated native packet"); return data[offset++]; }
    public int U16() => U8() << 8 | U8();
    public ulong U64() { ulong v = 0; for (var i = 0; i < 8; i++) v = v << 8 | U8(); return v; }
    /// <summary>The next <paramref name="count"/> bytes verbatim.</summary>
    public byte[] Bytes(int count) { if (count < 0 || Remaining < count) throw new InvalidDataException("Truncated native packet"); var b = data.Slice(offset, count).ToArray(); offset += count; return b; }
    public string Text(int count) { if (count < 0 || Remaining < count) throw new InvalidDataException("Truncated native packet"); var s = Encoding.UTF8.GetString(data.Slice(offset, count)); offset += count; return s; }
    public string Text() => Text(U16());
    public void Magic(string expected) { if (Text(4) != expected) throw new InvalidDataException("Unexpected native packet"); }
    public IReadOnlyList<ScoutQuest> Quests() => ScoutQuests.Parse(data, ref offset);
}

/// <summary>
/// Seed-code text handling: the as-you-type mask and the parser. Both are the
/// engine's own (<c>seedfinder_seed_format</c> / <c>seedfinder_seed_parse</c>
/// over <c>seed::format_input</c> and <c>DungeonSeed::from_code</c>), which is
/// what keeps the code the field shows and the value the search runs on the
/// game's rules — a locale-dependent C# uppercase, say, turned Turkish dotless
/// "\u0131" into an "I" the game never sees.
/// </summary>
public static class SeedCode
{
    /// <summary>
    /// Groups partial input as a seed code or daily date, detected from its first letter or digit.
    /// </summary>
    public static string Format(string value)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        var code = Native.seedfinder_seed_format(bytes, (nuint)bytes.Length, out var ptr, out var len);
        if (code != 0) throw new InvalidOperationException($"Native seed format failed ({code}).");
        return Encoding.UTF8.GetString(NativeEngine.CopyAndFree(ptr, len));
    }

    /// <summary>
    /// The canonical code and the numeric value of seed-code text, or null
    /// when the text does not name a seed.
    /// </summary>
    public static (string Code, ulong Value)? TryParse(string value)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        if (Native.seedfinder_seed_parse(bytes, (nuint)bytes.Length, out var ptr, out var len) != 0) return null;
        var document = JsonNode.Parse(Encoding.UTF8.GetString(NativeEngine.CopyAndFree(ptr, len))) as JsonObject
            ?? throw new InvalidDataException("Unreadable seed document");
        return ((string?)document["code"] ?? throw new InvalidDataException("Seed document has no code"),
            (ulong?)document["value"] ?? throw new InvalidDataException("Seed document has no value"));
    }

    /// <summary>
    /// Whether the text is already written the way the engine spells it: the
    /// canonical <c>XXX-XXX-XXX</c> form the field shows and files carry.
    /// </summary>
    public static bool IsCanonical(string value) => value.Length == 11 && IsScoutable(value);

    /// <summary>A canonical code or UTC daily date validated by the engine.</summary>
    public static bool IsScoutable(string value) => TryParse(value)?.Code == value;

    /// <summary>The numeric seed a code names.</summary>
    public static ulong Value(string value) => TryParse(value)?.Value
        ?? throw new ArgumentException($"Seed must use XXX-XXX-XXX format: {value}");
}

public sealed class NativeEngine
{
    public static string? ImpossibilityReason(QuerySettings query)
    {
        var packet = EncodeQuery(query);
        var code = Native.seedfinder_query_impossibility_reason(packet, (nuint)packet.Length, out var ptr, out var len);
        if (code != 0) throw new InvalidOperationException($"Native query analysis failed ({code}).");
        var reason = Encoding.UTF8.GetString(CopyAndFree(ptr, len));
        return reason.Length == 0 ? null : reason;
    }

    /// <summary>
    /// Logical processors available to search workers, never less than one:
    /// the ceiling for the worker selector. The engine's own count
    /// (<c>seedfinder_available_workers</c>), so the selector can never offer
    /// more threads than the search would actually spawn.
    /// </summary>
    public static int AvailableWorkers { get; } =
        (int)Math.Clamp(Native.seedfinder_available_workers(), 1u, int.MaxValue);

    /// <param name="workers">Search threads to spawn; the engine clamps it to
    /// <see cref="AvailableWorkers"/> and 0 means every available core.</param>
    public NativeSearch Start(QuerySettings query, int workers = 0)
    {
        var packet = EncodeQuery(query); var handle = Native.seedfinder_start_search(packet, (nuint)packet.Length, Workers(workers));
        if (handle == 0) throw new InvalidOperationException("The native engine rejected the query.");
        return new NativeSearch(handle);
    }

    /// <param name="workers">As in <see cref="Start"/>.</param>
    public NativeSearch StartResumed(QuerySettings query, long resumeFrom, long scanLength, int workers = 0)
    {
        var packet = EncodeQuery(query); var handle = Native.seedfinder_start_resumed_search(packet, (nuint)packet.Length, (ulong)resumeFrom, (ulong)scanLength, Workers(workers));
        if (handle == 0) throw new InvalidOperationException("The native engine rejected the query.");
        return new NativeSearch(handle);
    }


    /// <summary>A worker count as the FFI takes it: negatives, like 0, mean every core.</summary>
    private static uint Workers(int workers) => workers <= 0 ? 0u : (uint)workers;

    public IReadOnlyList<string> FilterSeeds(QuerySettings query, IReadOnlyList<string> seeds)
    {
        if (seeds.Count == 0) return [];
        var packet = EncodeQuery(query); var values = seeds.Select(SeedCode.Value).ToArray();
        var code = Native.seedfinder_filter_seeds(packet, (nuint)packet.Length, values, (nuint)values.Length, out var ptr, out var len);
        if (code != 0) throw new InvalidOperationException($"Native filter failed ({code}).");
        return ReadSeedList(CopyAndFree(ptr, len));
    }

    public IReadOnlyList<SeedResult> FilterRecipes(QuerySettings query, QuerySettings baseline, IReadOnlyList<SeedResult> recipes)
    {
        if (recipes.Count == 0) return [];
        var request = new JsonObject
        {
            ["query"] = JsonNode.Parse(ResultsExport.EncodeQueryDocument(query)),
            ["base_query"] = JsonNode.Parse(ResultsExport.EncodeQueryDocument(baseline)),
            ["trinkets"] = new JsonArray([.. recipes.Select(recipe => recipe.SelectedTrinket is string id ? JsonValue.Create(id) : null)]),
        };
        var packet = Encoding.UTF8.GetBytes(request.ToJsonString());
        var values = recipes.Select(recipe => SeedCode.Value(recipe.Seed)).ToArray();
        var code = Native.seedfinder_filter_seeds(packet, (nuint)packet.Length, values, (nuint)values.Length, out var ptr, out var len);
        if (code != 0) throw new InvalidOperationException($"Native filter failed ({code}).");
        return ReadRecipes(CopyAndFree(ptr, len));
    }


    /// <summary>
    /// The query as the engine takes it: the canonical JSON query document
    /// (the very bytes share links and results files carry), UTF-8. Every
    /// query-taking entry point accepts it, so there is one encoder here and
    /// the wire can never disagree with the documents.
    /// </summary>
    private static byte[] EncodeQuery(QuerySettings query) => Encoding.UTF8.GetBytes(ResultsExport.EncodeQueryDocument(query));


    /// <summary>The SSQ5 request naming one scouted world; scouting it is deterministic.</summary>
    internal static byte[] EncodeScoutRequest(string seed, int challenges, QuerySettings? query = null, string? trinket = null)
    {
        if (!SeedCode.IsScoutable(seed)) throw new ArgumentException("Choose a daily date or enter a XXX-XXX-XXX seed");
        var w = new Writer(); w.Bytes("SSQ6"u8.ToArray()); w.U16Le(challenges);
        var seedBytes = Encoding.UTF8.GetBytes(seed); w.U16Le(seedBytes.Length); w.Bytes(seedBytes);
        var overrideBytes = Encoding.UTF8.GetBytes(trinket ?? ""); w.U16Le(overrideBytes.Length); w.Bytes(overrideBytes);
        if (query is { HasRequirements: true }) w.Bytes(EncodeQuery(query));
        return w.Finish();
    }

    public ScoutWorld Scout(string seed, int challenges, QuerySettings? query = null, string? trinket = null)
    {
        var request = EncodeScoutRequest(seed, challenges, query, trinket);
        var code = Native.seedfinder_scout(request, (nuint)request.Length, out var ptr, out var len);
        if (code != 0) throw new InvalidOperationException($"Native scout failed ({code}).");
        return DecodeScout(CopyAndFree(ptr, len));
    }

    public static LevelMapDocument LevelMap(string requestJson)
    {
        var request = Encoding.UTF8.GetBytes(requestJson);
        var code = Native.seedfinder_level_map(request, (nuint)request.Length, out var ptr, out var len);
        if (code != 0) throw new InvalidOperationException(code == -1 ? "This floor or area is unavailable for the selected run." : "The engine could not generate this map.");
        return LevelMapDocument.Parse(CopyAndFree(ptr, len));
    }

    public static byte[] LevelMapAsset(string id)
    {
        var request = Encoding.UTF8.GetBytes(id);
        var code = Native.seedfinder_level_map_asset(request, (nuint)request.Length, out var ptr, out var len);
        if (code != 0) throw new InvalidOperationException($"The engine could not load map art ({id}).");
        return CopyAndFree(ptr, len);
    }

    public static ScoutWorld DecodeScout(byte[] bytes)
    {
        var r = new Reader(bytes); var version = r.Text(4);
        if (version is not ("SSC3" or "SSC4" or "SSC5" or "SSC6" or "SSC7" or "SSC8" or "SSC9")) throw new InvalidDataException("Unexpected scout packet");
        var returnedSeed = r.Text(r.U8()); var gems = new RingGems(r.Bytes(RingGems.Count));
        var quests = r.Quests(); var items = new List<ScoutItem>(); var count = r.U16();
        for (var i = 0; i < count; i++)
        {
            var item = ItemCatalog.Find(r.Text()) ?? throw new InvalidDataException("Unknown item in scout packet");
            var depth = r.U8(); var upgrade = r.U8(); var flags = r.U8(); var effect = r.Text();
            var source = (ScoutItemSource)r.U8(); var tag = r.U8(); var group = 0; ulong value = 0;
            if (tag == 1) { group = r.U16(); value = r.U8(); } else if (tag == 2) { group = r.U16(); value = r.U64(); } else if (tag != 0) throw new InvalidDataException("Unknown accessibility tag");
            items.Add(new(item, depth, upgrade, effect.Length == 0 ? null : effect, (flags & 1) != 0, source, tag, group, value, Secret: (flags & 2) != 0));
        }
        var order = new List<CatalogItem>();
        if (version is "SSC4" or "SSC5" or "SSC6" or "SSC7" or "SSC8" or "SSC9")
        {
            var orderCount = r.U8();
            if (orderCount != 17) throw new InvalidDataException("Unexpected trinket deck size");
            for (var i = 0; i < orderCount; i++)
            {
                var entry = ItemCatalog.Find(r.Text());
                if (entry is null || entry.Kind != ItemKind.Trinket || order.Any(x => x.Id == entry.Id))
                    throw new InvalidDataException("Invalid trinket deck");
                order.Add(entry);
            }
        }
        var feelings = new List<ScoutFloorFeeling>();
        if (version is "SSC5" or "SSC6" or "SSC7" or "SSC8" or "SSC9")
        {
            var feelingCount = r.U8();
            if (feelingCount > 20) throw new InvalidDataException("Unexpected floor feeling count");
            var previousDepth = 0;
            for (var i = 0; i < feelingCount; i++)
            {
                var depth = r.U8(); var feeling = r.U8();
                if (depth <= previousDepth || depth > 24 || depth % 5 == 0 || feeling > 7)
                    throw new InvalidDataException("Invalid floor feeling");
                feelings.Add(new(depth, (FloorFeeling)feeling));
                previousDepth = depth;
            }
        }
        var selectedTrinket = version is "SSC6" or "SSC7" or "SSC8" or "SSC9" ? r.Text() : "";
        if (selectedTrinket.Length == 0) selectedTrinket = null;
        if (selectedTrinket is not null && !order.Take(4).Any(item => item.Id == selectedTrinket))
            throw new InvalidDataException("Selected trinket is not initially offered");
        ScoutItemMappings? mappings = null;
        if (version is "SSC7" or "SSC8" or "SSC9")
        {
            mappings = new(ReadMappings(ref r, 304), ReadMappings(ref r, 352), ReadMappings(ref r, 224));
            if (!mappings.Rings.Select(entry => entry.SpriteIndex - 224).SequenceEqual(gems.Ordinals.Select(x => (int)x)))
                throw new InvalidDataException("Scout ring mappings disagree with run gems");
        }
        var floorRooms = new Dictionary<int, IReadOnlySet<string>>();
        if (version is "SSC8" or "SSC9")
        {
            var floorCount = r.U8();
            if (floorCount > 20) throw new InvalidDataException("Too many floor room summaries");
            var previousDepth = 0;
            for (var i = 0; i < floorCount; i++)
            {
                var depth = r.U8();
                if (depth <= previousDepth || depth > 24 || depth % 5 == 0)
                    throw new InvalidDataException("Room depths must be ascending regular floors 1..24");
                previousDepth = depth;
                var rooms = new HashSet<string>();
                var roomCount = r.U8();
                for (var j = 0; j < roomCount; j++)
                {
                    var room = r.Text();
                    if (room.Length == 0 || !rooms.Add(room)) throw new InvalidDataException("Empty or repeated floor room");
                }
                floorRooms.Add(depth, rooms);
            }
        }
        var artifactDecks = new Dictionary<int, IReadOnlyList<CatalogItem>>();
        if (version == "SSC9") {
            var deckCount = r.U8();
            if (deckCount > 24) throw new InvalidDataException("Too many artifact decks");
            var previous = 0;
            for (var i = 0; i < deckCount; i++) {
                var depth = r.U8();
                if (depth <= previous || depth > 24) throw new InvalidDataException("Invalid artifact floor");
                previous = depth;
                var size = r.U8();
                if (size > 11) throw new InvalidDataException("Too many artifacts");
                var deck = new List<CatalogItem>();
                for (var j = 0; j < size; j++) {
                    var id = r.Text();
                    var artifact = ItemCatalog.All.FirstOrDefault(item => item.Id == id && item.Kind == ItemKind.Artifact);
                    if (artifact is null || deck.Contains(artifact)) throw new InvalidDataException("Invalid artifact");
                    deck.Add(artifact);
                }
                artifactDecks.Add(depth, deck);
            }
        }
        if (r.Remaining != 0) throw new InvalidDataException("Trailing native data");
        return new(returnedSeed, quests, items, gems, order, feelings, selectedTrinket, mappings, floorRooms, artifactDecks);
    }

    private static IReadOnlyList<ScoutItemMapping> ReadMappings(ref Reader reader, int spriteBase)
    {
        var entries = new List<ScoutItemMapping>();
        for (var i = 0; i < 12; i++)
        {
            var name = reader.Text(); var appearance = reader.Text(); var sprite = reader.U16();
            if (string.IsNullOrWhiteSpace(name) || string.IsNullOrWhiteSpace(appearance)
                || sprite < spriteBase || sprite >= spriteBase + 12
                || entries.Any(entry => entry.Name == name || entry.Appearance == appearance || entry.SpriteIndex == sprite))
                throw new InvalidDataException("Invalid scout item mapping");
            entries.Add(new(name, appearance, sprite));
        }
        return entries;
    }

    /// <summary>
    /// Which items of the world scouted by <paramref name="seed"/> and
    /// <paramref name="challenges"/> satisfy <paramref name="query"/>, as
    /// indices into the item list <see cref="Scout"/> returns for the same
    /// request. The engine owns the selection — the very matcher the search
    /// runs, so a marked manifest can never disagree with the result list —
    /// and it is asked over the same SSQ5 request bytes, which name the world
    /// exactly.
    /// </summary>
    public static ScoutMatches ScoutMatches(string seed, int challenges, QuerySettings query, string? trinket = null)
    {
        var request = EncodeScoutRequest(seed, challenges, query, trinket); var packet = EncodeQuery(query);
        var code = Native.seedfinder_scout_matches(request, (nuint)request.Length, packet, (nuint)packet.Length, out var ptr, out var len);
        // A query the engine cannot decode — one with no requirements, which
        // the scout pane shows a manifest for anyway — marks nothing. Counts
        // are slots: an "any of these" group is one requirement.
        var slots = query.SlotCount;
        if (code == -1) return new(new HashSet<int>(), 0, slots);
        if (code != 0) throw new InvalidOperationException($"Native scout matches failed ({code}).");
        var document = JsonNode.Parse(Encoding.UTF8.GetString(CopyAndFree(ptr, len))) as JsonObject
            ?? throw new InvalidDataException("Unreadable scout match document");
        var matched = new HashSet<int>();
        foreach (var index in document["matched"] as JsonArray ?? [])
            if (index is JsonValue value && value.TryGetValue(out int number)) matched.Add(number);
        return new(matched, (int?)document["matchedRequirements"] ?? matched.Count,
            (int?)document["totalRequirements"] ?? slots) {
                TransmutedTrinkets = (document["transmutedTrinkets"] as JsonArray ?? []).Select(value => (int)value!).ToHashSet(),
                TransmutedArtifacts = (document["transmutedArtifacts"] as JsonArray ?? []).Select(value => ((int)value!["depth"]!, (int)value!["index"]!)).ToHashSet()
            };
    }

    /// <summary>The full web share link for a canonical JSON query document, or null when the engine rejects the query.</summary>
    public static string? TryEncodeShareLink(string queryJson)
    {
        var bytes = Encoding.UTF8.GetBytes(queryJson);
        return Native.seedfinder_share_encode(bytes, (nuint)bytes.Length, out var ptr, out var len) == 0
            ? Encoding.UTF8.GetString(CopyAndFree(ptr, len)) : null;
    }

    /// <summary>The canonical JSON query document carried by share-link text (web link, seedseeker:// link, or bare code), or null when there is none.</summary>
    public static string? TryDecodeShareText(string text)
    {
        var bytes = Encoding.UTF8.GetBytes(text);
        return Native.seedfinder_share_decode(bytes, (nuint)bytes.Length, out var ptr, out var len) == 0
            ? Encoding.UTF8.GetString(CopyAndFree(ptr, len)) : null;
    }

    /// <summary>
    /// The results-file text for the UTF-8 JSON request
    /// <c>{"query", "seeds", "app_version"}</c>, or null when the engine
    /// rejects it. The file schema and every validation rule are the core
    /// codec's (crates/seedfinder-core/src/results_export.rs).
    /// </summary>
    public static string? TryEncodeResultsFile(string requestJson)
    {
        var bytes = Encoding.UTF8.GetBytes(requestJson);
        return Native.seedfinder_results_encode(bytes, (nuint)bytes.Length, out var ptr, out var len) == 0
            ? Encoding.UTF8.GetString(CopyAndFree(ptr, len)) : null;
    }

    /// <summary>
    /// The UTF-8 JSON <c>{"query", "seeds", "dropped", "app_version",
    /// "shpd_version"}</c> a results file carries — seeds already deduplicated
    /// and capped by the engine — or null when the text is not an importable
    /// results file.
    /// </summary>
    public static string? TryDecodeResultsFile(string contents)
    {
        var bytes = Encoding.UTF8.GetBytes(contents);
        return Native.seedfinder_results_decode(bytes, (nuint)bytes.Length, out var ptr, out var len) == 0
            ? Encoding.UTF8.GetString(CopyAndFree(ptr, len)) : null;
    }

    /// <summary>The engine's own constants document, as UTF-8 JSON.</summary>
    public static string EngineInfoJson()
    {
        var code = Native.seedfinder_engine_info(out var ptr, out var len);
        if (code != 0) throw new InvalidOperationException($"Native engine info failed ({code}).");
        return Encoding.UTF8.GetString(CopyAndFree(ptr, len));
    }

    internal static byte[] CopyAndFree(nint ptr, nuint len)
    {
        try { var bytes = new byte[(int)len]; Marshal.Copy(ptr, bytes, 0, bytes.Length); return bytes; }
        finally { if (ptr != 0) Native.seedfinder_buffer_free(ptr, len); }
    }

    internal static IReadOnlyList<string> ReadSeedList(byte[] bytes) => ReadRecipes(bytes).Select(recipe => recipe.Seed).ToArray();

    internal static IReadOnlyList<SeedResult> ReadRecipes(byte[] bytes)
    {
        var r = new Reader(bytes); var magic = r.Text(4);
        if (magic is not ("SSR1" or "SSR2")) throw new InvalidDataException("Unexpected native packet");
        var count = r.U16(); var result = new List<SeedResult>(count);
        for (var i = 0; i < count; i++)
        {
            var seed = r.Text(r.U8());
            var trinket = magic == "SSR2" ? r.Text() : "";
            if (trinket.Length > 0 && ItemCatalog.Find(trinket)?.Kind != ItemKind.Trinket)
                throw new InvalidDataException("Unknown trinket in native packet");
            result.Add(new(seed, i + 1, trinket.Length == 0 ? null : trinket));
        }
        if (r.Remaining != 0) throw new InvalidDataException("Trailing native result data");
        return result;
    }

}

/// <summary>
/// The engine's own constants, read once from <c>seedfinder_engine_info</c>
/// instead of mirrored in C#: the pinned upstream version and commit, and the
/// limits the shared codecs enforce.
/// </summary>
public static class EngineInfo
{
    private static readonly JsonObject Document =
        JsonNode.Parse(NativeEngine.EngineInfoJson()) as JsonObject
        ?? throw new InvalidOperationException("The engine returned an unreadable info document.");

    /// <summary>The upstream Shattered Pixel Dungeon version this engine reproduces.</summary>
    public static string ShpdVersion { get; } = Text("shpdVersion");

    /// <summary>The upstream commit the reproduction was ported from.</summary>
    public static string ShpdCommit { get; } = Text("shpdCommit");

    /// <summary>The import cap the results codec enforces on file text.</summary>
    public static int ResultsFileMaxBytes { get; } = Limit("resultsFileMaxBytes");

    public static IReadOnlyList<int> MapDepths { get; } = Document["levelMaps"]!["supportedDepths"]!.AsArray().Select(x => (int)x!).ToArray();
    public static string MapRevision { get; } = $"{ShpdCommit}/{Document["levelMaps"]!["schemaVersion"]}/{Document["levelMaps"]!["assetRevision"]}";

    private static string Text(string key) => (string?)Document[key]
        ?? throw new InvalidOperationException($"The engine info document has no \"{key}\".");

    private static int Limit(string key) => (int?)(Document["limits"] as JsonObject)?[key]
        ?? throw new InvalidOperationException($"The engine info document has no limit \"{key}\".");
}

public sealed class NativeSearch : IDisposable
{
    private long handle;
    internal NativeSearch(long value) => handle = value;
    public IReadOnlyList<string> Poll(int maximum) => PollRecipes(maximum).Select(recipe => recipe.Seed).ToArray();
    public IReadOnlyList<SeedResult> PollRecipes(int maximum)
    {
        var code = Native.seedfinder_poll(handle, (uint)maximum, out var ptr, out var len);
        if (code != 0) throw new InvalidOperationException($"Native poll failed ({code}).");
        return NativeEngine.ReadRecipes(NativeEngine.CopyAndFree(ptr, len));
    }
    public SearchStatus Status()
    {
        var raw = new long[5]; var code = Native.seedfinder_status(handle, raw); if (code != 0) throw new InvalidOperationException($"Native status failed ({code}).");
        return new((SearchState)raw[0], raw[1], raw[2], raw[3], BitConverter.Int64BitsToDouble(raw[4]));
    }
    public (long ResumeFrom, long Remaining) ResumeHint()
    {
        var hint = new long[2]; var code = Native.seedfinder_resume_hint(handle, hint); if (code != 0) throw new InvalidOperationException($"Native resume hint failed ({code}).");
        return (hint[0], hint[1]);
    }
    public void Cancel() { if (handle != 0) Native.seedfinder_cancel(handle); }
    public void Dispose() { var old = Interlocked.Exchange(ref handle, 0); if (old != 0) Native.seedfinder_close(old); GC.SuppressFinalize(this); }
    ~NativeSearch() => Dispose();
}
