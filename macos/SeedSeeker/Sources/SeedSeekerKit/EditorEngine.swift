import CSeedFinder
import Foundation

/// The transport to the requirement editor's rules.
///
/// Every rule behind the requirement board and sheet — how the flat list
/// folds into chips, what a drag, a badge or a menu writes back, what each
/// chip says and what is wrong with a list — lives once, in the shared core
/// (`crates/seedfinder-core/src/editor/`, documented in
/// `docs/requirement-editor.md`). The app reaches it through two stateless
/// JSON envelopes; like the other document helpers they only transform
/// bytes, so they run synchronously on the caller's thread.
enum EditorEngine {
    /// The board envelope's answer to `request`, or nil when the request
    /// cannot be sent or the core answers it with an error document.
    static func board(_ request: [String: Any]) -> [String: Any]? {
        answer(request, send: boardText)
    }

    /// The sheet envelope's answer to `request`, or nil as for ``board(_:)``.
    static func editor(_ request: [String: Any]) -> [String: Any]? {
        answer(request, send: editorText)
    }

    /// `seedfinder_requirement_board` on request bytes: the answer's JSON
    /// bytes, error documents included, or nil when the call itself failed.
    static func boardText(_ request: Data) -> Data? {
        try? enginePacket { out, length in
            request.withUnsafeBytes { bytes in
                seedfinder_requirement_board(
                    bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count, out, length)
            }
        }
    }

    /// `seedfinder_requirement_editor` on request bytes, as ``boardText(_:)``.
    static func editorText(_ request: Data) -> Data? {
        try? enginePacket { out, length in
            request.withUnsafeBytes { bytes in
                seedfinder_requirement_editor(
                    bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count, out, length)
            }
        }
    }

    private static func answer(_ request: [String: Any], send: (Data) -> Data?) -> [String: Any]? {
        // An object JSONSerialization cannot write raises rather than throws,
        // so it is checked first.
        guard JSONSerialization.isValidJSONObject(request),
              let data = try? JSONSerialization.data(withJSONObject: request),
              let packet = send(data),
              let answer = (try? JSONSerialization.jsonObject(with: packet)) as? [String: Any],
              answer["error"] == nil else { return nil }
        return answer
    }
}

// MARK: - Rows

extension ResultsExport {
    /// One row of the editor's list: the requirement exactly as the query
    /// document writes it, plus the row's key and its either/or label, which
    /// the document spells as an `any_of` entry instead.
    static func encodeRow(_ requirement: ItemRequirement) -> [String: Any] {
        var row = encodeRequirement(requirement)
        row["key"] = requirement.key
        if let group = requirement.alternativeGroup { row["alternative_group"] = group }
        return row
    }

    /// The requirement a row the editor wrote holds, or nil when it is not
    /// one this build can model.
    static func decodeRow(_ value: Any) -> ItemRequirement? {
        guard let row = value as? [String: Any],
              let key = (row["key"] as? NSNumber).flatMap({ Int64(exactly: $0) }) else { return nil }
        let group = (row["alternative_group"] as? NSNumber).flatMap { Int(exactly: $0) }
        return try? decodeRequirement(row, key: key, alternativeGroup: group)
    }

    /// The rows of an answer, or nil unless every one is a requirement this
    /// build can model: a list is taken whole or not at all.
    static func decodeRows(_ value: Any?) -> [ItemRequirement]? {
        guard let written = value as? [Any] else { return nil }
        var rows: [ItemRequirement] = []
        for value in written {
            guard let row = decodeRow(value) else { return nil }
            rows.append(row)
        }
        return rows
    }

    /// The kind a document name (`melee_weapon`) stands for.
    static func kind(named name: String?) -> ItemKind? {
        guard let name, let index = kindNames.firstIndex(of: name) else { return nil }
        return ItemKind(rawValue: index)
    }
}

// MARK: - Loading

extension Array where Element == ItemRequirement {
    /// A loaded or imported list as the board edits it: keyed in order and
    /// rewritten once into the canonical encoding (the core's `normalize`).
    public func loadedForBoard() -> [ItemRequirement] {
        let keyed = withKeysInOrder()
        guard let result = RequirementBoard.apply([.normalize], to: keyed), result.changed else { return keyed }
        return result.rows
    }
}

extension SavedQuery {
    /// The query as a load or import hands it to the board: its requirements
    /// keyed in order and in the canonical encoding.
    public func loadedForBoard() -> SavedQuery {
        var loaded = self
        loaded.requirements = requirements.loadedForBoard()
        return loaded
    }

    /// The query's Arcane Resin condition as the board shows it.
    public var boardResin: BoardResin? {
        BoardResin(amount: arcaneResin, auto: arcaneResinAuto, filter: arcaneResinFilter)
    }

    /// The requirement board of the query as it stands, from the shared core
    /// and memoized per change of the requirements (see
    /// ``RequirementBoard/of(_:resin:)``).
    public var board: RequirementBoard {
        RequirementBoard.of(requirements, resin: boardResin)
    }
}
