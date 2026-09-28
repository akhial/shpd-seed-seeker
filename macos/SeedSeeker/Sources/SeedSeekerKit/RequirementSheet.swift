import Foundation

/**
 The requirement sheet — the dialog a chip opens into — as the shared core
 holds it (`docs/requirement-editor.md`, "The sheet"): an opaque draft the
 app sends back untouched, and the form it shows.

 Every rule of the sheet is the core's: which controls show, what they offer
 and between which bounds, what a category switch resets, what a save writes
 and whether it may. The app draws ``form``, sends each control the user
 moves as a ``SheetChange`` and saves the draft onto the list as it is then.
 Each call is one synchronous request, made when the user acts — never from
 a view body.
 */
public struct RequirementSheet: Hashable, Sendable {
    /// The draft, as the core wrote it; only the core reads it.
    let draft: String
    /// Everything the sheet shows for the draft.
    public let form: SheetForm

    /// The item picker's value for Arcane Resin, which the sheet offers
    /// among the wands when asked to.
    public static let arcaneResin = "arcane_resin"

    /// Opens the sheet on `rows`, the list as it stands: on the visible row
    /// `key`, on a new chip in the section `blanket` picks when `key` is nil,
    /// or on the query's resin condition with `openResin`. `resin` is the
    /// query's condition, which seeds the resin section; `offerResin` offers
    /// Arcane Resin among the wands. Nil when the core cannot open it.
    public static func open(rows: [ItemRequirement], key: Int64? = nil, blanket: Bool = false,
                            resin: BoardResin? = nil, offerResin: Bool = false,
                            openResin: Bool = false) -> RequirementSheet? {
        var request: [String: Any] = ["op": "open", "rows": rows.map(ResultsExport.encodeRow),
                                      "blanket": blanket, "offer_resin": offerResin,
                                      "open_resin": openResin]
        if let key { request["key"] = key }
        if let resin { request["resin"] = resin.object }
        guard let answer = EditorEngine.editor(request) else { return nil }
        return RequirementSheet(json: answer)
    }

    /// The sheet after the user moved one control, or nil when the core
    /// could not apply it (the sheet then stays as it was).
    public func changing(_ change: SheetChange) -> RequirementSheet? {
        let request: [String: Any] = ["op": "change", "draft": draft, "change": change.object]
        guard let answer = EditorEngine.editor(request) else { return nil }
        return RequirementSheet(json: answer)
    }

    /// Saves the draft onto `rows`, the list as it is now: the rows and the
    /// query's resin it comes to, or the sheet again with the reasons in its
    /// form's errors. Nil when the core could not be asked.
    public func save(onto rows: [ItemRequirement]) -> SheetSaveOutcome? {
        let request: [String: Any] = ["op": "save", "draft": draft,
                                      "rows": rows.map(ResultsExport.encodeRow)]
        guard let answer = EditorEngine.editor(request) else { return nil }
        if let saved = answer["saved"] as? [String: Any] {
            guard let result = SheetSaved(json: saved, sent: rows) else { return nil }
            return .saved(result)
        }
        guard let refused = RequirementSheet(json: answer) else { return nil }
        return .refused(refused)
    }

    /// The sheet of an open or change answer, `{"draft", "form"}`.
    init?(json answer: [String: Any]) {
        guard let draft = answer["draft"] as? String, let form = SheetForm(json: answer["form"]) else { return nil }
        self.init(draft: draft, form: form)
    }

    init(draft: String, form: SheetForm) {
        self.draft = draft
        self.form = form
    }
}

/// What saving a sheet came to.
public enum SheetSaveOutcome: Sendable {
    /// The list and the query's resin after the save.
    case saved(SheetSaved)
    /// The draft cannot be saved; the sheet's form says why.
    case refused(RequirementSheet)
}

/// A save the core made: the rows to write back — only when ``changed`` —
/// and what becomes of the query's Arcane Resin condition.
public struct SheetSaved: Sendable {
    /// The core's rows when ``changed``, else the very rows that were sent.
    public let rows: [ItemRequirement]
    public let changed: Bool
    /// Keys the core had to repair on the way.
    public let rekeyed: [BoardKeyChange]
    /// The chip the save landed in; nil when it saved the resin.
    public let focus: Int64?
    public let resin: SheetResinOutcome

    /// The key a row had before the core repaired it now goes by.
    public func key(following old: Int64) -> Int64 {
        rekeyed.first(where: { $0.from == old })?.to ?? old
    }

    /// The saved answer to a request that sent `rows`, or nil when its rows
    /// or resin are not ones this build can model.
    init?(json object: [String: Any], sent rows: [ItemRequirement]) {
        changed = jsonFlag(object["changed"])
        if changed {
            guard let written = ResultsExport.decodeRows(object["rows"]) else { return nil }
            self.rows = written
        } else {
            self.rows = rows
        }
        rekeyed = jsonKeyChanges(object["rekeyed"])
        focus = jsonKey(object["focus"])
        let resin = object["resin"] as? [String: Any] ?? [:]
        if let set = resin["set"] {
            guard let condition = BoardResin(json: set) else { return nil }
            self.resin = .set(condition)
        } else if jsonFlag(resin["clear"]) {
            self.resin = .clear
        } else {
            self.resin = .unchanged
        }
    }
}

/// What a save does to the query's Arcane Resin condition.
public enum SheetResinOutcome: Hashable, Sendable {
    /// The sheet was about an item.
    case unchanged
    /// Arcane Resin was the picked item: the query asks for this condition.
    case set(BoardResin)
    /// The resin chip was saved as an item: the query drops its resin.
    case clear
}

extension BoardResin {
    /// A condition as the envelopes write one (RESIN):
    /// `{"amount": N | "auto", "filter": {...}}`. An Auto condition keeps no
    /// amount of its own, as the saved query stores it.
    init?(json value: Any?) {
        guard let object = value as? [String: Any] else { return nil }
        let filter = object["filter"] as? [String: Any] ?? [:]
        var uncursed = true
        if filter["uncursed"] is NSNumber { uncursed = jsonFlag(filter["uncursed"]) }
        let source = jsonString(filter["source"])
            .flatMap { ResultsExport.sourceNames.firstIndex(of: $0) }
            .flatMap { ScoutItemSource(rawValue: $0) }
        let auto = jsonString(object["amount"]) == "auto"
        self.init(amount: auto ? 0 : jsonInt(object["amount"]) ?? 0, auto: auto,
                  filter: ArcaneResinFilter(uncursed: uncursed, maximumDepth: jsonInt(filter["max_depth"]),
                                            source: source, includeMageWand: jsonFlag(filter["include_mage_wand"])))
    }
}

// MARK: - Changes

/// One control the user moved (`docs/requirement-editor.md`, "CHANGE").
/// Pickers send back the `value` of the option chosen, exactly as the form
/// listed it; numbers go as the control shows them, and the core clamps
/// them into range. A change to a control the form hides changes nothing.
public enum SheetChange: Sendable {
    /// A family: `weapon`, `armor`, `wand`, `ring`, `trinket`, `artifact`.
    case category(String)
    /// `any`, `melee` or `thrown`.
    case weaponType(String)
    /// An item's catalog id, nil for the wildcard, or
    /// ``RequirementSheet/arcaneResin``.
    case item(String?)
    /// `any`, `exact`, `at_least`, `at_most`.
    case tierMode(String)
    case tier(Int)
    /// `any`, `exact`, `at_least`.
    case upgradeMode(String)
    case upgrade(Int)
    /// `any`, `any_enchantment`, `specific`.
    case effectMode(String)
    /// Ticks or unticks one effect of the "Specific…" grid.
    case toggleEffect(String)
    case uncursed(Bool)
    /// A source's document name, nil for any source.
    case source(String?)
    case floorLimitEnabled(Bool)
    case floorLimit(Int)
    case excludeResin(Bool)
    case transmutationsEnabled(Bool)
    case transmutations(Int)
    case selectTrinket(Bool)
    case count(Int)
    case copyDepthEnabled(Bool)
    case copyDepth(Int)
    case countLevels(Bool)
    case total(Int)
    case resinAuto(Bool)
    /// The amount as typed; nil for an empty field or text that is no number.
    case resinAmount(Double?)
    case includeMageWand(Bool)

    var object: [String: Any] {
        switch self {
        case .category(let family): return ["type": "set_category", "value": family]
        case .weaponType(let type): return ["type": "set_weapon_type", "value": type]
        case .item(let item): return ["type": "set_item", "value": nullable(item)]
        case .tierMode(let mode): return ["type": "set_tier_mode", "value": mode]
        case .tier(let tier): return ["type": "set_tier", "value": wireByte(tier)]
        case .upgradeMode(let mode): return ["type": "set_upgrade_mode", "value": mode]
        case .upgrade(let upgrade): return ["type": "set_upgrade", "value": wireByte(upgrade)]
        case .effectMode(let mode): return ["type": "set_effect_mode", "value": mode]
        case .toggleEffect(let effect): return ["type": "toggle_effect", "value": effect]
        case .uncursed(let on): return ["type": "set_uncursed", "value": on]
        case .source(let source): return ["type": "set_source", "value": nullable(source)]
        case .floorLimitEnabled(let on): return ["type": "set_floor_limit_enabled", "value": on]
        case .floorLimit(let depth): return ["type": "set_floor_limit", "value": wireByte(depth)]
        case .excludeResin(let on): return ["type": "set_exclude_resin", "value": on]
        case .transmutationsEnabled(let on): return ["type": "set_transmutations_enabled", "value": on]
        case .transmutations(let count): return ["type": "set_transmutations", "value": wireByte(count)]
        case .selectTrinket(let on): return ["type": "set_select_trinket", "value": on]
        case .count(let count): return ["type": "set_count", "value": wireByte(count)]
        case .copyDepthEnabled(let on): return ["type": "set_copy_depth_enabled", "value": on]
        case .copyDepth(let depth): return ["type": "set_copy_depth", "value": wireByte(depth)]
        case .countLevels(let on): return ["type": "set_count_levels", "value": on]
        case .total(let total): return ["type": "set_total", "value": wireByte(total)]
        case .resinAuto(let on): return ["type": "set_resin_auto", "value": on]
        case .resinAmount(let amount):
            // JSON has no infinity: text like "inf" is no amount either.
            var value: Any = NSNull()
            if let amount, amount.isFinite { value = amount }
            return ["type": "set_resin_amount", "value": value]
        case .includeMageWand(let on): return ["type": "set_include_mage_wand", "value": on]
        }
    }
}

// MARK: - The form

/// Everything the sheet shows (`docs/requirement-editor.md`, "FORM"). The
/// dialog chrome — its title and button words — is the app's, and follows
/// ``mode``, ``blanket``, ``inCluster`` and ``resinPicked``.
public struct SheetForm: Hashable, Sendable {
    public enum Mode: String, Hashable, Sendable {
        case new, edit
    }

    /// What the sheet was opened on.
    public enum Origin: Hashable, Sendable {
        /// A new chip.
        case new
        /// The visible row with this key.
        case row(Int64)
        /// The query's Arcane Resin chip.
        case resin
    }

    public let mode: Mode
    public let origin: Origin
    public let blanket: Bool
    /// The row is a member of an either/or cluster, whose stack is the
    /// cluster's.
    public let inCluster: Bool
    /// The sheet edits the query's resin rather than an item.
    public let resinPicked: Bool
    /// The requirement's title (`Any Tier 3+ melee weapon`), or `Arcane
    /// Resin`; there even while the draft has errors.
    public let title: String
    /// The chip a save would put on the board (key 0, no join candidates),
    /// or nil while the draft has errors or the resin is picked.
    public let preview: BoardChip?
    /// The six families.
    public let category: SheetPicker
    /// The eight kinds, melee and thrown weapons among them.
    public let kind: SheetPicker
    /// Any, melee or thrown, on weapons.
    public let weaponType: SheetPicker
    /// The wildcard, Arcane Resin when offered, then the items — weapons
    /// under `Tier 2`…`Tier 5`.
    public let item: SheetPicker
    public let tier: SheetModeRange
    public let upgrade: SheetModeRange
    public let effect: SheetEffect
    public let uncursed: SheetToggle
    public let source: SheetPicker
    public let floorLimit: SheetFloorToggle
    public let excludeResin: SheetToggle
    public let transmutations: SheetRangeToggle
    public let selectTrinket: SheetToggle
    public let stack: SheetStack
    public let resin: SheetResin
    /// Why the draft cannot be saved, in the order to show them.
    public let errors: [String]
    public let canSave: Bool

    /// Every control reads defensively: one the answer leaves out is hidden.
    init?(json value: Any?) {
        guard let object = value as? [String: Any] else { return nil }
        mode = jsonString(object["mode"]) == "new" ? .new : .edit
        let origin = object["origin"] as? [String: Any] ?? [:]
        let originType = jsonString(origin["type"])
        if originType == "resin" {
            self.origin = .resin
        } else if originType == "row", let key = jsonKey(origin["key"]) {
            self.origin = .row(key)
        } else {
            self.origin = .new
        }
        blanket = jsonFlag(object["blanket"])
        inCluster = jsonFlag(object["in_cluster"])
        resinPicked = jsonFlag(object["resin_picked"])
        title = jsonString(object["title"]) ?? ""
        preview = (object["preview"] as? [String: Any]).flatMap { BoardChip(json: $0) }
        category = SheetPicker(json: object["category"])
        kind = SheetPicker(json: object["kind"])
        weaponType = SheetPicker(json: object["weapon_type"])
        item = SheetPicker(json: object["item"])
        tier = SheetModeRange(json: object["tier"])
        upgrade = SheetModeRange(json: object["upgrade"])
        effect = SheetEffect(json: object["effect"])
        uncursed = SheetToggle(json: object["uncursed"])
        source = SheetPicker(json: object["source"])
        floorLimit = SheetFloorToggle(json: object["floor_limit"])
        excludeResin = SheetToggle(json: object["exclude_resin"])
        transmutations = SheetRangeToggle(json: object["transmutations"])
        selectTrinket = SheetToggle(json: object["select_trinket"])
        stack = SheetStack(json: object["stack"])
        resin = SheetResin(json: object["resin"])
        errors = jsonStrings(object["errors"])
        canSave = jsonFlag(object["can_save"])
    }
}

/// One choice of a picker.
public struct SheetOption: Hashable, Identifiable, Sendable {
    /// What ``SheetChange`` sends back: a catalog id, a family, a kind, a
    /// mode or a source name; nil for the wildcard item or any source.
    public let value: String?
    public let label: String
    /// The heading the choice sits under (`Tier 3`).
    public let group: String?
    /// Offered only because the draft already names it (a tier-1 item from
    /// an imported query).
    public let hidden: Bool

    public var id: String { value ?? "" }
    /// A catalog item rather than the wildcard or Arcane Resin.
    public var isCatalogItem: Bool { value != nil && value != RequirementSheet.arcaneResin }
    public var isArcaneResin: Bool { value == RequirementSheet.arcaneResin }

    init?(json object: [String: Any]) {
        guard let label = jsonString(object["label"]) else { return nil }
        value = jsonString(object["value"])
        self.label = label
        group = jsonString(object["group"])
        hidden = jsonFlag(object["hidden"])
    }
}

/// A run of a picker's options under one heading, or under none.
public struct SheetOptionSection: Hashable, Identifiable, Sendable {
    /// The position among the picker's sections.
    public let id: Int
    public let title: String?
    public let options: [SheetOption]
}

/// A picker: its value and its choices.
public struct SheetPicker: Hashable, Sendable {
    public let visible: Bool
    public let value: String?
    public let options: [SheetOption]

    /// The option shown as chosen.
    public var selected: SheetOption? { options.first(where: { $0.value == value }) }

    /// The options in order, each run sharing a heading as one section.
    public var sections: [SheetOptionSection] {
        var sections: [SheetOptionSection] = []
        var run: [SheetOption] = []
        for option in options {
            if let last = run.last, last.group != option.group {
                sections.append(SheetOptionSection(id: sections.count, title: last.group, options: run))
                run = []
            }
            run.append(option)
        }
        if let last = run.last {
            sections.append(SheetOptionSection(id: sections.count, title: last.group, options: run))
        }
        return sections
    }

    init(json value: Any?) {
        let object = value as? [String: Any] ?? [:]
        visible = jsonFlag(object["visible"])
        self.value = jsonString(object["value"])
        options = jsonObjects(object["options"]).compactMap { SheetOption(json: $0) }
    }
}

/// A mode picker with a value slider (tier, upgrade). The value is within
/// `min...max` even while the mode is `any` and the slider hidden.
public struct SheetModeRange: Hashable, Sendable {
    public let visible: Bool
    public let mode: String
    public let modes: [SheetOption]
    public let value: Int
    public let min: Int
    public let max: Int
    /// The value in words: `Tier 3 or higher`, `+2`.
    public let valueLabel: String

    /// Whether the mode takes a value at all.
    public var hasValue: Bool { mode != "any" }
    /// The chosen mode's own word (`At least`).
    public var modeLabel: String { modes.first(where: { $0.value == mode })?.label ?? "" }
    /// Whether the slider has anywhere to go.
    public var isAdjustable: Bool { max > min }

    init(json value: Any?) {
        let object = value as? [String: Any] ?? [:]
        visible = jsonFlag(object["visible"])
        mode = jsonString(object["mode"]) ?? "any"
        modes = jsonObjects(object["modes"]).compactMap { SheetOption(json: $0) }
        let min = jsonInt(object["min"]) ?? 0
        self.value = jsonInt(object["value"]) ?? min
        self.min = min
        max = Swift.max(min, jsonInt(object["max"]) ?? min)
        valueLabel = jsonString(object["value_label"]) ?? ""
    }
}

/// One effect of the "Specific…" grid.
public struct SheetEffectChoice: Hashable, Identifiable, Sendable {
    /// The effect's name, which ``SheetChange/toggleEffect(_:)`` sends.
    public let value: String
    public let label: String
    /// `enchantment` or `curse`.
    public let group: String
    public let selected: Bool

    public var id: String { value }

    init?(json object: [String: Any]) {
        guard let value = jsonString(object["value"]) else { return nil }
        self.value = value
        label = jsonString(object["label"]) ?? value
        group = jsonString(object["group"]) ?? ""
        selected = jsonFlag(object["selected"])
    }
}

/// The effect filter of a weapon or armor.
public struct SheetEffect: Hashable, Sendable {
    public let visible: Bool
    public let mode: String
    /// Any, Any enchantment (Any glyph), Specific….
    public let modes: [SheetOption]
    /// The family's effects in catalog order; curses only while the item
    /// may be cursed.
    public let choices: [SheetEffectChoice]
    /// The grid's headings, one per group listed.
    public let groups: [SheetOption]
    /// What the ticked effects mean.
    public let caption: String

    /// Whether the grid of effects shows.
    public var isSpecific: Bool { mode == "specific" }

    /// The choices under one heading of ``groups``.
    public func choices(in group: String?) -> [SheetEffectChoice] {
        choices.filter { $0.group == group }
    }

    init(json value: Any?) {
        let object = value as? [String: Any] ?? [:]
        visible = jsonFlag(object["visible"])
        mode = jsonString(object["mode"]) ?? "any"
        modes = jsonObjects(object["modes"]).compactMap { SheetOption(json: $0) }
        choices = jsonObjects(object["choices"]).compactMap { SheetEffectChoice(json: $0) }
        groups = jsonObjects(object["groups"]).compactMap { SheetOption(json: $0) }
        caption = jsonString(object["caption"]) ?? ""
    }
}

/// A check box.
public struct SheetToggle: Hashable, Sendable {
    public let visible: Bool
    public let value: Bool
    public let label: String

    init(json value: Any?) {
        let object = value as? [String: Any] ?? [:]
        visible = jsonFlag(object["visible"])
        self.value = jsonFlag(object["value"])
        label = jsonString(object["label"]) ?? ""
    }
}

/// A switch with a floor slider (the item's floor limit, the copies', the
/// resin donors'). The value is always one of ``options``, which skip the
/// empty boss floors.
public struct SheetFloorToggle: Hashable, Sendable {
    public let visible: Bool
    public let enabled: Bool
    public let value: Int
    /// The floors the slider stops at.
    public let options: [Int]
    public let label: String
    /// `Within first 4 floors`.
    public let valueLabel: String

    /// The slider position of the value among ``options``.
    public var index: Int {
        options.firstIndex(of: value) ?? options.lastIndex(where: { $0 < value }) ?? 0
    }

    /// The floor at a slider position, or nil when there are no floors.
    public func floor(at index: Int) -> Int? {
        guard !options.isEmpty else { return nil }
        return options[Swift.min(Swift.max(index, 0), options.count - 1)]
    }

    init(json value: Any?) {
        let object = value as? [String: Any] ?? [:]
        visible = jsonFlag(object["visible"])
        enabled = jsonFlag(object["enabled"])
        self.value = jsonInt(object["value"]) ?? 0
        options = jsonObjects(object["options"]).compactMap { jsonInt($0["value"]) }
        label = jsonString(object["label"]) ?? ""
        valueLabel = jsonString(object["value_label"]) ?? ""
    }
}

/// A switch with a stepper (transmutations, a combined level). The value is
/// always within `min...max`.
public struct SheetRangeToggle: Hashable, Sendable {
    public let visible: Bool
    public let enabled: Bool
    public let value: Int
    public let min: Int
    public let max: Int
    public let label: String
    public let caption: String?
    public let valueLabel: String

    public var range: ClosedRange<Int> { min...max }
    public var isAdjustable: Bool { max > min }

    init(json value: Any?) {
        let object = value as? [String: Any] ?? [:]
        visible = jsonFlag(object["visible"])
        enabled = jsonFlag(object["enabled"])
        let min = jsonInt(object["min"]) ?? 1
        self.value = jsonInt(object["value"]) ?? min
        self.min = min
        max = Swift.max(min, jsonInt(object["max"]) ?? min)
        label = jsonString(object["label"]) ?? ""
        caption = jsonString(object["caption"])
        valueLabel = jsonString(object["value_label"]) ?? ""
    }
}

/// The stack section: how many items, the copies' floor limit and the
/// combined level.
public struct SheetStack: Hashable, Sendable {
    public let visible: Bool
    public let count: Int
    public let min: Int
    public let max: Int
    /// `×2`.
    public let valueLabel: String
    public let copyDepth: SheetFloorToggle
    public let countLevels: SheetRangeToggle

    public var range: ClosedRange<Int> { min...max }

    init(json value: Any?) {
        let object = value as? [String: Any] ?? [:]
        visible = jsonFlag(object["visible"])
        let min = jsonInt(object["min"]) ?? 1
        count = jsonInt(object["count"]) ?? min
        self.min = min
        max = Swift.max(min, jsonInt(object["max"]) ?? min)
        valueLabel = jsonString(object["value_label"]) ?? ""
        copyDepth = SheetFloorToggle(json: object["copy_depth"])
        countLevels = SheetRangeToggle(json: object["count_levels"])
    }
}

/// The Arcane Resin section, shown while the resin is picked. Its donors'
/// filter is edited through the form's uncursed, source and floor controls.
public struct SheetResin: Hashable, Sendable {
    public let visible: Bool
    public let auto: Bool
    /// The amount as typed, nil for an empty field.
    public let amount: Double?
    public let includeMageWand: Bool

    /// The amount when it is a whole number, for a stepper.
    public var wholeAmount: Int? {
        guard let amount, amount.isFinite, amount.rounded() == amount,
              abs(amount) < 1_000_000_000 else { return nil }
        return Int(amount)
    }

    /// The amount as a text field starts from.
    public var amountText: String {
        if let whole = wholeAmount { return String(whole) }
        return amount.map { String($0) } ?? ""
    }

    init(json value: Any?) {
        let object = value as? [String: Any] ?? [:]
        visible = jsonFlag(object["visible"])
        auto = jsonFlag(object["auto"])
        amount = jsonNumber(object["amount"])
        includeMageWand = jsonFlag(object["include_mage_wand"])
    }
}
