import Foundation

/// One board edit (`docs/requirement-editor.md`, "EDIT"). Keys name visible
/// rows — a chip, or one member of a cluster — never a stack's hidden copies;
/// an edit naming a key the list does not show changes nothing.
public enum BoardEdit: Sendable {
    /// Rewrites the list into its canonical encoding, once, as it is loaded.
    case normalize
    /// Makes `source` an either/or alternative of `target`.
    case join(source: Int64, target: Int64)
    /// Takes a cluster member out on its own; it leaves the stack behind.
    case detach(Int64)
    /// Removes a cluster member, or a chip's whole entry.
    case remove(Int64)
    /// Removes the whole entry holding the row: members and hidden copies.
    case removeItem(Int64)
    /// How many items the chip asks for — a lone chip or one cluster member.
    case setCount(Int64, Int)
    /// Sets or clears a lone ring stack's combined level.
    case setTotal(Int64, Int?)
    /// Turns counting levels on or off.
    case toggleLevels(Int64)
    /// Sets or clears the floor limit of the chip's hidden copies.
    case setCopyDepth(Int64, Int?)

    var object: [String: Any] {
        switch self {
        case .normalize:
            return ["type": "normalize"]
        case .join(let source, let target):
            return ["type": "join", "source": source, "target": target]
        case .detach(let key):
            return ["type": "detach", "key": key]
        case .remove(let key):
            return ["type": "remove", "key": key]
        case .removeItem(let key):
            return ["type": "remove_item", "key": key]
        case .setCount(let key, let count):
            return ["type": "set_count", "key": key, "count": wireByte(count)]
        case .setTotal(let key, let total):
            return ["type": "set_total", "key": key, "total": nullable(total.map(wireByte))]
        case .toggleLevels(let key):
            return ["type": "toggle_levels", "key": key]
        case .setCopyDepth(let key, let depth):
            return ["type": "set_copy_depth", "key": key, "max_depth": nullable(depth.map(wireByte))]
        }
    }
}

/// The query's Arcane Resin condition, which the board shows as its own chip.
public struct BoardResin: Hashable, Sendable {
    public let auto: Bool
    public let amount: Int
    public let filter: ArcaneResinFilter

    /// The condition, or nil when the query asks for no resin: no Auto and
    /// no amount. The amount's bounds are the query's own (its model keeps
    /// to the format's) and the sheet's, which the core words.
    public init?(amount: Int, auto: Bool, filter: ArcaneResinFilter) {
        guard auto || amount > 0, filter.isValid else { return nil }
        self.auto = auto; self.amount = amount; self.filter = filter
    }

    var object: [String: Any] {
        var wireFilter: [String: Any] = ["uncursed": filter.uncursed,
                                         "include_mage_wand": filter.includeMageWand]
        if let depth = filter.maximumDepth { wireFilter["max_depth"] = depth }
        if let source = filter.source { wireFilter["source"] = ResultsExport.sourceNames[source.rawValue] }
        var object: [String: Any] = ["filter": wireFilter]
        if auto { object["amount"] = "auto" } else { object["amount"] = amount }
        return object
    }
}

// MARK: - The board

/**
 The requirement board as the shared core draws it: the flat requirement list
 folded into chips, either/or clusters and stacks, with every word a chip, a
 badge or a problem shows, and what each chip may join.

 ``of(_:resin:)`` answers the board of a list as it stands and is memoized per
 list, so a view body may ask for it as often as it likes; ``apply(_:to:resin:)``
 runs edits. Rows are named by their `key` throughout — the core keeps keys
 stable across every edit.
 */
public struct RequirementBoard: Sendable {
    /// The list after the edits: the core's rows when ``changed``, else the
    /// very rows that were sent.
    public let rows: [ItemRequirement]
    /// Whether the rows differ from those sent. Only then are they written
    /// back; an edit that did nothing leaves the list — and every document
    /// encoded from it — untouched.
    public let changed: Bool
    /// Keys the core had to repair, a zero or repeated key say. Lists are
    /// keyed in order as they load, so this is normally empty.
    public let rekeyed: [BoardKeyChange]
    /// The row to follow after the edits: the joined or detached row, or the
    /// entry an edit reshaped. Nil after a removal and when no edit applied.
    public let focus: Int64?
    /// Why an edit was refused; the edits before it still applied.
    public let refusal: BoardRefusal?
    /// The board's entries in list order, both sections together.
    public let items: [BoardItem]
    /// How many entries each section shows; a cluster or stack counts once.
    public let ordinaryCount: Int
    public let blanketCount: Int
    /// Everything wrong with the list: each row's own problems, then those
    /// between rows, then the list's.
    public let problems: [BoardProblem]
    /// The Arcane Resin chip, when the request carried the query's resin.
    public let resin: BoardResinChip?

    /// The entries of one section.
    public func section(blanket: Bool) -> [BoardItem] {
        items.filter { $0.blanket == blanket }
    }

    /// The entry showing the visible row `key`.
    public func item(holding key: Int64) -> BoardItem? {
        items.first { $0.members.contains(key) }
    }

    /// The chip of the visible row `key`.
    public func chip(_ key: Int64) -> BoardChip? {
        for item in items {
            if let chip = item.chips.first(where: { $0.key == key }) { return chip }
        }
        return nil
    }

    /// The key a row had before the core repaired it now goes by.
    public func key(following old: Int64) -> Int64 {
        rekeyed.first { $0.from == old }?.to ?? old
    }

    // MARK: Asking the core

    /// The board of `rows` as they stand. Memoized per list and resin
    /// condition, so re-rendering, dragging and hovering never call the core
    /// again until the list itself changes.
    public static func of(_ rows: [ItemRequirement], resin: BoardResin? = nil) -> RequirementBoard {
        let key = BoardCacheKey(rows: rows, resin: resin)
        if let cached = cache.board(for: key) { return cached }
        guard let board = request(edits: [], rows: rows, resin: resin) else { return unavailable(rows) }
        cache.store(board, for: key)
        return board
    }

    /// Runs `edits` in order on `rows`, or nil when the core could not be
    /// asked. Callers write ``rows`` back only when ``changed``.
    public static func apply(_ edits: [BoardEdit], to rows: [ItemRequirement],
                             resin: BoardResin? = nil) -> RequirementBoard? {
        guard let result = request(edits: edits, rows: rows, resin: resin) else { return nil }
        // The answer already draws the list it returns, which is the list the
        // caller renders next.
        cache.store(result.settled, for: BoardCacheKey(rows: result.rows, resin: resin))
        return result
    }

    private static let cache = BoardCache()

    private static func request(edits: [BoardEdit], rows: [ItemRequirement],
                                resin: BoardResin?) -> RequirementBoard? {
        var request: [String: Any] = ["rows": rows.map(ResultsExport.encodeRow)]
        if !edits.isEmpty { request["edits"] = edits.map(\.object) }
        if let resin { request["resin"] = resin.object }
        guard let answer = EditorEngine.board(request) else { return nil }
        return decode(answer, sent: rows)
    }

    /// The typed board of a board envelope's answer to a request that sent
    /// `rows`, or nil when its rows are not ones this build can model — an
    /// answer is taken whole or not at all.
    static func decode(_ answer: [String: Any], sent rows: [ItemRequirement]) -> RequirementBoard? {
        let changed = jsonFlag(answer["changed"])
        var list = rows
        if changed {
            guard let written = ResultsExport.decodeRows(answer["rows"]) else { return nil }
            list = written
        }
        let counts = answer["counts"] as? [String: Any] ?? [:]
        return RequirementBoard(
            rows: list, changed: changed, rekeyed: jsonKeyChanges(answer["rekeyed"]),
            focus: jsonKey(answer["focus"]),
            refusal: BoardRefusal(json: answer["refused"]),
            items: jsonObjects(answer["items"]).compactMap(BoardItem.init(json:)),
            ordinaryCount: jsonInt(counts["ordinary"]) ?? 0,
            blanketCount: jsonInt(counts["blanket"]) ?? 0,
            problems: jsonObjects(answer["problems"]).compactMap(BoardProblem.init(json:)),
            resin: BoardResinChip(json: answer["resin"]))
    }

    /// The board as a later look at the same rows sees it: no edit of its own.
    private var settled: RequirementBoard {
        RequirementBoard(rows: rows, changed: false, rekeyed: [], focus: nil, refusal: nil,
                         items: items, ordinaryCount: ordinaryCount, blanketCount: blanketCount,
                         problems: problems, resin: resin)
    }

    /// What is shown should the core ever fail to answer: no chips, and a
    /// problem that keeps the search from starting on a list nobody can see.
    private static func unavailable(_ rows: [ItemRequirement]) -> RequirementBoard {
        RequirementBoard(rows: rows, changed: false, rekeyed: [], focus: nil, refusal: nil,
                         items: [], ordinaryCount: 0, blanketCount: 0,
                         problems: [BoardProblem(message: "The requirements could not be read.",
                                                 keys: [], scope: .list)],
                         resin: nil)
    }
}

/// A key the core repaired, and the key the row goes by now.
public struct BoardKeyChange: Hashable, Sendable {
    public let from: Int64
    public let to: Int64
}

/// Why an edit was refused.
public struct BoardRefusal: Hashable, Sendable {
    /// Stable: `blanket_total` or `no_free_group`.
    public let reason: String
    /// The sentence to show.
    public let message: String

    init?(json value: Any?) {
        guard let object = value as? [String: Any], let message = jsonString(object["message"]) else { return nil }
        reason = jsonString(object["reason"]) ?? ""
        self.message = message
    }
}

/// One problem with the list and the rows it blames.
public struct BoardProblem: Hashable, Sendable {
    public enum Scope: String, Sendable {
        /// One requirement on its own.
        case row
        /// Requirements that disagree: a stack, a combined level, a cluster.
        case group
        /// The list as a whole; it blames no row.
        case list
    }

    public let message: String
    /// The rows at fault, hidden copies included, in list order.
    public let keys: [Int64]
    public let scope: Scope

    init(message: String, keys: [Int64], scope: Scope) {
        self.message = message; self.keys = keys; self.scope = scope
    }

    init?(json object: [String: Any]) {
        guard let message = jsonString(object["message"]) else { return nil }
        self.message = message
        keys = jsonKeys(object["keys"])
        scope = jsonString(object["scope"]).flatMap(Scope.init(rawValue:)) ?? .list
    }
}

// MARK: - Entries and chips

/// One board entry: a chip, or an either/or cluster of chips. Stacks and
/// their badges belong to the chips, a cluster's members included; nothing
/// is counted per entry.
public struct BoardItem: Hashable, Identifiable, Sendable {
    /// `r17` for a chip, `c3` for a cluster; stable while the entry
    /// survives an edit.
    public let id: String
    public let blanket: Bool
    /// The cluster's either/or label, nil for a chip.
    public let cluster: Int?
    /// The cluster's caption (`Any of 3`).
    public let label: String?
    /// The entry's name where a menu names it ("Either/or with…"): a chip's
    /// name, or a cluster's members' names joined (`Spear or Mace`).
    public let name: String
    /// The visible rows: one for a chip, every member of a cluster.
    public let members: [Int64]
    /// Every hidden copy of the entry: those behind each chip's badge.
    public let extras: [Int64]
    /// One chip per member.
    public let chips: [BoardChip]
    /// The first problem touching any member or hidden copy.
    public let problem: String?

    /// The entry's first visible row, which names it where a view needs one.
    public var anchor: Int64 { members[0] }

    init?(json object: [String: Any]) {
        let members = jsonKeys(object["members"])
        let chips = jsonObjects(object["chips"]).compactMap(BoardChip.init(json:))
        guard let id = jsonString(object["id"]), !members.isEmpty, !chips.isEmpty else { return nil }
        self.id = id
        blanket = jsonFlag(object["blanket"])
        cluster = jsonInt(object["cluster"])
        label = jsonString(object["label"])
        name = jsonString(object["name"]) ?? chips[0].name
        self.members = members
        extras = jsonKeys(object["extras"])
        self.chips = chips
        problem = jsonString(object["problem"])
    }
}

/// What a chip's count, combined-level and copy-floor steppers offer: its
/// own stack, whether it stands alone or is a cluster member.
public struct BoardStack: Hashable, Sendable {
    /// How many items the chip asks for, itself included.
    public let count: Int
    /// The most items any stack may ask for.
    public let max: Int
    /// Whether the chip can grow a stack at all.
    public let canGrow: Bool
    /// Whether the count stepper is live: the chip can grow, or it has
    /// copies to shed.
    public let canChangeCount: Bool
    /// The count stepper's upper bound: ``max`` while the chip can grow,
    /// else its count, which it may only shed copies from.
    public let countMax: Int
    /// The combined level, when the stack counts levels.
    public let total: Int?
    /// Whether "count levels together" applies (or can be turned off):
    /// only a ring stack standing on its own, never a cluster member.
    public let canCountLevels: Bool
    /// The total stepper's upper bound.
    public let levelCapacity: Int
    /// Where the total starts when counting is turned on.
    public let defaultTotal: Int
    /// The hidden copies' floor limit.
    public let copyDepth: Int?
    public let canSetCopyDepth: Bool
    /// `×2`, or `≤2` while counting levels — there even at ×1, for steppers.
    public let countText: String
    /// `Σ ≥ 5`.
    public let totalText: String

    /// The counts the stepper offers: one up to the core's ``countMax``.
    public var countRange: ClosedRange<Int> { 1...Swift.max(1, countMax) }
    /// The totals the combined-level stepper offers.
    public var totalRange: ClosedRange<Int> { 1...Swift.max(1, levelCapacity) }

    /// Missing or unreadable, the stack is one item with nothing to step:
    /// a chip is never dropped for want of its stack.
    init(json value: Any?) {
        let object = value as? [String: Any] ?? [:]
        let count = jsonInt(object["count"]) ?? 1
        self.count = count
        max = jsonInt(object["max"]) ?? count
        canGrow = jsonFlag(object["can_grow"])
        canChangeCount = jsonFlag(object["can_change_count"])
        // Missing, the stepper may only shed copies: never more than asked.
        countMax = jsonInt(object["count_max"]) ?? count
        total = jsonInt(object["total"])
        canCountLevels = jsonFlag(object["can_count_levels"])
        levelCapacity = jsonInt(object["level_capacity"]) ?? 1
        defaultTotal = jsonInt(object["default_total"]) ?? 1
        copyDepth = jsonInt(object["copy_depth"])
        canSetCopyDepth = jsonFlag(object["can_set_copy_depth"])
        countText = jsonString(object["count_text"]) ?? ""
        totalText = jsonString(object["total_text"]) ?? ""
    }
}

/// A badge at rest: its text, the compact form narrow layouts use, and what
/// it means.
public struct BoardBadge: Hashable, Sendable {
    public let text: String
    public let compactText: String
    public let tooltip: String

    init?(json value: Any?) {
        guard let object = value as? [String: Any], let text = jsonString(object["text"]) else { return nil }
        self.text = text
        compactText = jsonString(object["compact_text"]) ?? text
        tooltip = jsonString(object["tooltip"]) ?? ""
    }
}

/// One visible row: a chip on its own, or one member of a cluster.
public struct BoardChip: Hashable, Identifiable, Sendable {
    public let key: Int64
    /// The short name beside the sprite: the item, or `Any melee`.
    public let name: String
    /// The full title a popover or sheet leads with.
    public let title: String
    /// The item's catalog id, nil for a wildcard.
    public let item: String?
    /// The catalog entry the sprite draws, nil for a wildcard.
    public let catalogItem: CatalogItem?
    /// The kind (`meleeWeapon`) and family (`weapon`) the sprite follows.
    public let kind: ItemKind?
    public let family: ItemKind?
    /// Qualifiers after the name, in order.
    public let tags: [ChipTag]
    /// Qualifiers after the effect cue (`No resin`).
    public let trailingTags: [ChipTag]
    /// The effect the row asks for; nil for any effect.
    public let effect: ChipEffect?
    /// Cursed items are ruled out.
    public let uncursed: Bool
    /// The popover's detail line, as parts.
    public let details: [String]
    /// The popover's relation lines.
    public let relations: [ChipRelation]
    /// The accessibility label: the title, then the details.
    public let description: String
    /// The row's own first problem, else the first one between rows blaming it.
    public let problem: String?
    /// The badges the chip shows at rest: the count (`×3`) when it asks for
    /// more than one item, the combined level (`Σ ≥ 5`) when it counts
    /// levels. A cluster member's are its own; a picked-up chip shows
    /// neither, since a drag moves one item.
    public let countBadge: BoardBadge?
    public let totalBadge: BoardBadge?
    /// The hidden copies behind the chip's count badge. Members whose stacks
    /// are alike share theirs.
    public let copies: [Int64]
    /// What the chip's steppers offer.
    public let stack: BoardStack
    public let inCluster: Bool
    /// Whether "on its own" applies.
    public let canDetach: Bool
    /// The visible rows this chip may join, in list order.
    public let join: [Int64]
    /// The visible rows a join onto is refused, and why.
    public let refuse: [JoinRefusal]

    public var id: Int64 { key }

    /// Why joining this chip onto `target` is refused, if it is.
    public func refusal(onto target: Int64) -> JoinRefusal? {
        refuse.first { $0.key == target }
    }

    init?(json object: [String: Any]) {
        guard let key = jsonKey(object["key"]), let name = jsonString(object["name"]) else { return nil }
        let title = jsonString(object["title"]) ?? name
        let item = jsonString(object["item"])
        self.key = key
        self.name = name
        self.title = title
        self.item = item
        catalogItem = item.flatMap { ItemCatalog.findById($0) }
        kind = ResultsExport.kind(named: jsonString(object["kind"]))
        family = ResultsExport.kind(named: jsonString(object["family"]))
        tags = jsonObjects(object["tags"]).compactMap(ChipTag.init(json:))
        trailingTags = jsonObjects(object["trailing_tags"]).compactMap(ChipTag.init(json:))
        effect = ChipEffect(json: object["effect"])
        uncursed = jsonFlag(object["uncursed"])
        details = jsonStrings(object["details"])
        relations = jsonObjects(object["relations"]).compactMap(ChipRelation.init(json:))
        description = jsonString(object["description"]) ?? title
        problem = jsonString(object["problem"])
        let badges = object["badges"] as? [String: Any] ?? [:]
        countBadge = BoardBadge(json: badges["count"])
        totalBadge = BoardBadge(json: badges["total"])
        copies = jsonKeys(object["copies"])
        stack = BoardStack(json: object["stack"])
        inCluster = jsonFlag(object["in_cluster"])
        canDetach = jsonFlag(object["can_detach"])
        join = jsonKeys(object["join"])
        refuse = jsonObjects(object["refuse"]).compactMap(JoinRefusal.init(json:))
    }
}

/// A qualifier beside a chip's name, tinted by its ``style``.
public struct ChipTag: Hashable, Sendable {
    public enum Style: String, Hashable, Sendable {
        /// A filter: the tier, a floor, `No resin`.
        case plain
        /// The upgrade (`+3`, `+3↑`).
        case upgrade
        /// Resin the resin chip counts: its amount and `Mage +2`, tinted
        /// apart from its donor filter.
        case credit
    }

    public let text: String
    public let style: Style
    /// The tag's own hover text (what Auto means, where `Mage +2` comes
    /// from), or nil; a chip's tags have none.
    public let tooltip: String?

    public var isUpgrade: Bool { style == .upgrade }
    public var isCredit: Bool { style == .credit }

    init?(json object: [String: Any]) {
        guard let text = jsonString(object["text"]) else { return nil }
        self.text = text
        // A style this build does not know is drawn plain.
        style = jsonString(object["style"]).flatMap(Style.init(rawValue:)) ?? .plain
        tooltip = jsonString(object["tooltip"])
    }
}

/// The effect a chip asks for.
public struct ChipEffect: Hashable, Sendable {
    /// `any enchantment`, `any glyph`, one effect's name, or `effect: A/B`.
    public let label: String
    /// The effects in catalog order; the family's full set for any enchantment.
    public let effects: [String]
    public let anyEnchantment: Bool
    public let cursesOnly: Bool

    /// The effects whose glows the sprite pulses through. "Any enchantment"
    /// settles on no one colour, so it has none.
    public var glowNames: [String] { anyEnchantment ? [] : effects }

    init?(json value: Any?) {
        guard let object = value as? [String: Any], let label = jsonString(object["label"]) else { return nil }
        self.label = label
        effects = jsonStrings(object["effects"])
        anyEnchantment = jsonFlag(object["any_enchantment"])
        cursesOnly = jsonFlag(object["curses_only"])
    }
}

/// One relation line of a chip's popover, led by the glyph of what it relates.
public struct ChipRelation: Hashable, Sendable {
    public enum Glyph: String, Sendable {
        /// The cluster's other members.
        case or
        /// The combined level.
        case sum
        /// The stack's copies.
        case times
    }

    public let glyph: Glyph
    public let text: String

    init?(json object: [String: Any]) {
        guard let glyph = jsonString(object["glyph"]).flatMap(Glyph.init(rawValue:)),
              let text = jsonString(object["text"]) else { return nil }
        self.glyph = glyph
        self.text = text
    }
}

/// A visible row a chip may not join, and why.
public struct JoinRefusal: Hashable, Sendable {
    public let key: Int64
    public let reason: String
    public let message: String

    init?(json object: [String: Any]) {
        guard let key = jsonKey(object["key"]), let message = jsonString(object["message"]) else { return nil }
        self.key = key
        reason = jsonString(object["reason"]) ?? ""
        self.message = message
    }
}

/// The Arcane Resin chip: a query-level requirement the board draws beside
/// the items, which joins and stacks nothing.
public struct BoardResinChip: Hashable, Sendable {
    public let name: String
    /// `Auto` or `≥N` — always there, always first — then `Mage +2`, both
    /// styled ``ChipTag/Style/credit`` with their own tooltips, then `F≤N`.
    public let tags: [ChipTag]
    public let uncursed: Bool
    /// The chip's hover text: the donors' source, or nil for any source.
    public let tooltip: String?
    public let details: [String]
    /// The accessibility label.
    public let description: String

    init?(json value: Any?) {
        guard let object = value as? [String: Any], let name = jsonString(object["name"]) else { return nil }
        self.name = name
        tags = jsonObjects(object["tags"]).compactMap(ChipTag.init(json:))
        uncursed = jsonFlag(object["uncursed"])
        tooltip = jsonString(object["tooltip"])
        details = jsonStrings(object["details"])
        description = jsonString(object["description"]) ?? name
    }
}

// MARK: - Reading the answers

// Every field has one JSON type. Numbers and booleans both arrive as
// NSNumber, which converts either way, so each reader checks which one it
// holds rather than trusting the cast. Internal, not private: the sheet's
// answers (`RequirementSheet`) are read the same way.

func jsonString(_ value: Any?) -> String? {
    value as? String
}

private func jsonIsBoolean(_ number: NSNumber) -> Bool {
    CFGetTypeID(number as CFTypeRef) == CFBooleanGetTypeID()
}

func jsonInt(_ value: Any?) -> Int? {
    guard let number = value as? NSNumber, !jsonIsBoolean(number) else { return nil }
    return Int(exactly: number)
}

/// A number that may have a fraction: the resin amount as typed.
func jsonNumber(_ value: Any?) -> Double? {
    guard let number = value as? NSNumber, !jsonIsBoolean(number) else { return nil }
    return number.doubleValue
}

func jsonKey(_ value: Any?) -> Int64? {
    guard let number = value as? NSNumber, !jsonIsBoolean(number) else { return nil }
    return Int64(exactly: number)
}

func jsonFlag(_ value: Any?) -> Bool {
    guard let number = value as? NSNumber, jsonIsBoolean(number) else { return false }
    return number.boolValue
}

/// A true or false that is a value rather than a flag (an option's value):
/// nil unless the answer holds JSON's true or false.
func jsonBool(_ value: Any?) -> Bool? {
    guard let number = value as? NSNumber, jsonIsBoolean(number) else { return nil }
    return number.boolValue
}

func jsonKeys(_ value: Any?) -> [Int64] {
    (value as? [Any] ?? []).compactMap { jsonKey($0) }
}

func jsonStrings(_ value: Any?) -> [String] {
    (value as? [Any] ?? []).compactMap { $0 as? String }
}

func jsonObjects(_ value: Any?) -> [[String: Any]] {
    (value as? [Any] ?? []).compactMap { $0 as? [String: Any] }
}

/// The `rekeyed` pairs of an answer: each repaired key and its new one.
func jsonKeyChanges(_ value: Any?) -> [BoardKeyChange] {
    (value as? [Any] ?? []).compactMap { pair -> BoardKeyChange? in
        let keys = jsonKeys(pair)
        guard keys.count == 2 else { return nil }
        return BoardKeyChange(from: keys[0], to: keys[1])
    }
}

/// A board request's inputs, what the memo is keyed by.
private struct BoardCacheKey: Equatable {
    let rows: [ItemRequirement]
    let resin: BoardResin?
}

/// The last few boards asked for. A handful rather than one: the two board
/// sections, the header counts and the footer all ask while an edit's own
/// answer is stored ahead of the render that needs it.
private final class BoardCache: @unchecked Sendable {
    private let lock = NSLock()
    private var entries: [(key: BoardCacheKey, board: RequirementBoard)] = []
    private let capacity = 4

    func board(for key: BoardCacheKey) -> RequirementBoard? {
        lock.lock(); defer { lock.unlock() }
        guard let index = entries.firstIndex(where: { $0.key == key }) else { return nil }
        let entry = entries.remove(at: index)
        entries.insert(entry, at: 0)
        return entry.board
    }

    func store(_ board: RequirementBoard, for key: BoardCacheKey) {
        lock.lock(); defer { lock.unlock() }
        entries.removeAll { $0.key == key }
        entries.insert((key: key, board: board), at: 0)
        if entries.count > capacity { entries.removeLast(entries.count - capacity) }
    }
}

/// A count, level or floor as the envelopes read one: a byte.
func wireByte(_ value: Int) -> Int {
    min(max(value, 0), 255)
}

/// The value, or JSON's null.
func nullable<Value>(_ value: Value?) -> Any {
    if let value { return value }
    return NSNull()
}
