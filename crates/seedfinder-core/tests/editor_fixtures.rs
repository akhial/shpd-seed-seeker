//! Golden request/response pairs for the requirement editor's JSON
//! envelopes (`docs/requirement-editor.md`).
//!
//! Every platform that reaches the editor through the envelopes — the web
//! through WebAssembly, the Apple and Windows apps through the C ABI,
//! Android through JNI — decodes these files in its own tests, so a change to
//! the wire format shows up here first, as a diff, rather than as a platform
//! that quietly reads a field that moved. The test regenerates every pair
//! and fails when a file drifts; run it with `UPDATE_EDITOR_FIXTURES=1` to
//! rewrite them after a deliberate change:
//!
//! ```sh
//! UPDATE_EDITOR_FIXTURES=1 cargo test -p shpd-seedfinder-core --test editor_fixtures
//! ```
//!
//! Each file is `{"about", "envelope", "request", "response"}`. A request
//! is sent as its JSON text — except a request stored as a string, which is
//! sent as that string (the fixtures for text that is not JSON at all).
//! Sheet requests that carry a draft carry the very string an earlier
//! response returned.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use shpd_seedfinder_core::editor::{requirement_board, requirement_editor};

#[derive(Clone, Copy)]
enum Envelope {
    Board,
    Editor,
}

impl Envelope {
    const fn name(self) -> &'static str {
        match self {
            Self::Board => "requirement_board",
            Self::Editor => "requirement_editor",
        }
    }

    /// Sends `request` as a platform would: its JSON text, or — for a
    /// fixture pinning a request that is not JSON at all — the string
    /// itself.
    fn call(self, request: &Value) -> Value {
        let text = match request {
            Value::String(text) => text.clone(),
            request => request.to_string(),
        };
        let response = match self {
            Self::Board => requirement_board(&text),
            Self::Editor => requirement_editor(&text),
        };
        serde_json::from_str(&response).expect("the envelopes answer JSON")
    }
}

/// The fixtures, built in order: a sheet request can carry the draft an
/// earlier sheet response returned.
#[derive(Default)]
struct Fixtures {
    files: Vec<(String, Value)>,
}

impl Fixtures {
    /// Records one pair and returns its response.
    fn add(&mut self, name: &str, about: &str, envelope: Envelope, request: &Value) -> Value {
        let response = envelope.call(request);
        self.files.push((
            format!("{name}.json"),
            json!({
                "about": about,
                "envelope": envelope.name(),
                "request": request,
                "response": response,
            }),
        ));
        response
    }
}

/// A representative board: a concrete stack, a narrowed wildcard with an
/// effect and floor, an either/or cluster, a trinket with transmutations and
/// a blanket.
fn tour() -> Value {
    json!([
        {"key": 1, "kind": "ring", "item": "ring_might", "upgrade": 2},
        {"key": 2, "kind": "ring", "item": "ring_might"},
        {"key": 3, "kind": "ring", "item": "ring_might"},
        {"key": 4, "kind": "melee_weapon", "tier": {"at_least": 3},
         "upgrade": {"at_least": 2}, "effect": "any_enchantment", "uncursed": true,
         "max_depth": 9},
        {"key": 5, "kind": "wand", "item": "wand_fireblast", "upgrade": 3, "alternative_group": 1},
        {"key": 6, "kind": "wand", "upgrade": 3, "exclude_resin": true, "alternative_group": 1},
        {"key": 7, "kind": "trinket", "item": "rat_skull", "trinket_transmutations": 3},
        {"key": 8, "kind": "armor", "effect": ["Brimstone", "Viscosity"], "source": "locked_chest",
         "blanket": true},
    ])
}

/// The list a user reported: Wand of Disintegration ×2 (keys 1 and 20)
/// beside a Wand of Frost (key 7).
fn repro() -> Value {
    json!([
        {"key": 1, "kind": "wand", "item": "wand_disintegration"},
        {"key": 2, "kind": "wand", "item": "wand_lightning"},
        {"key": 3, "kind": "ring", "item": "ring_energy", "upgrade": 4},
        {"key": 4, "kind": "ring", "item": "ring_energy", "max_depth": 20},
        {"key": 5, "kind": "ring", "item": "ring_energy", "max_depth": 20},
        {"key": 6, "kind": "armor", "item": "plate_armor", "upgrade": 3},
        {"key": 7, "kind": "wand", "item": "wand_frost"},
        {"key": 8, "kind": "wand", "upgrade": 3},
        {"key": 20, "kind": "wand", "item": "wand_disintegration"},
    ])
}

#[allow(clippy::too_many_lines)] // One pair after another, in reading order.
fn board_fixtures(fixtures: &mut Fixtures) {
    use Envelope::Board;
    fixtures.add(
        "board-empty",
        "An empty board.",
        Board,
        &json!({"rows": []}),
    );
    fixtures.add(
        "board-tour",
        "Every kind of entry, with the Auto resin chip; no edits, so the rows come back unchanged, in canonical spelling.",
        Board,
        &json!({
            "rows": tour(),
            "resin": {"amount": "auto", "filter": {"uncursed": true, "max_depth": null,
                      "source": "heap", "include_mage_wand": true}},
        }),
    );
    fixtures.add(
        "board-resin-credit",
        "A fixed resin amount with the Mage's credit and a donor floor: the resin the chip counts is styled credit, Mage +2 explains itself, the floor is a plain filter, and the source is the chip's hover text.",
        Board,
        &json!({
            "rows": [{"key": 1, "kind": "wand", "item": "wand_frost"}],
            "resin": {"amount": 4, "filter": {"uncursed": true, "max_depth": 9,
                      "source": "locked_chest", "include_mage_wand": true}},
        }),
    );
    fixtures.add(
        "board-stack-concrete",
        "A +2 Ring of Might grown to three: plain repeats (the first web stack document).",
        Board,
        &json!({
            "rows": [{"key": 1, "kind": "ring", "item": "ring_might", "upgrade": 2}],
            "edits": [{"type": "set_count", "key": 1, "count": 3}],
        }),
    );
    fixtures.add(
        "board-stack-wildcard",
        "A +3 wand of any kind grown to three: bare copies sharing an identity group (the second web stack document).",
        Board,
        &json!({
            "rows": [{"key": 1, "kind": "wand", "upgrade": 3}],
            "edits": [{"type": "set_count", "key": 1, "count": 3}],
        }),
    );
    fixtures.add(
        "board-stack-total",
        "Two Rings of Might counting levels to at least 3 (the third web stack document).",
        Board,
        &json!({
            "rows": [{"key": 1, "kind": "ring", "item": "ring_might", "upgrade": 2}],
            "edits": [
                {"type": "set_count", "key": 1, "count": 2},
                {"type": "set_total", "key": 1, "total": 3},
            ],
        }),
    );
    fixtures.add(
        "board-stack-cluster",
        "A wand joined to a Wand of Fireblast, each member grown to three: their alike stacks share one label and one set of copies (the fourth web stack document), and each chip shows ×3.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "upgrade": 3},
                {"key": 2, "kind": "wand", "item": "wand_fireblast", "upgrade": 3},
            ],
            "edits": [
                {"type": "join", "source": 1, "target": 2},
                {"type": "set_count", "key": 1, "count": 3},
                {"type": "set_count", "key": 2, "count": 3},
            ],
        }),
    );
    fixtures.add(
        "board-stack-member",
        "The +3 wand of (Fireblast or any +3 wand) grown to two: a stack of its own, under a label only it carries — two of the same wand, or a Fireblast.",
        Board,
        &json!({
            "rows": [
                {"key": 2, "kind": "wand", "item": "wand_fireblast", "upgrade": 3, "alternative_group": 1},
                {"key": 1, "kind": "wand", "upgrade": 3, "alternative_group": 1},
            ],
            "edits": [{"type": "set_count", "key": 1, "count": 2}],
        }),
    );
    fixtures.add(
        "board-cluster-alike-stacks",
        "(Frost or Disintegration or Lightning) with one label on Frost and Disintegration and one copy: the cluster draws no badge of its own; Frost and Disintegration each show ×2, Lightning none.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "item": "wand_frost", "alternative_group": 1, "identity_group": 1},
                {"key": 2, "kind": "wand", "item": "wand_disintegration", "alternative_group": 1, "identity_group": 1},
                {"key": 3, "kind": "wand", "item": "wand_lightning", "alternative_group": 1},
                {"key": 4, "kind": "wand", "identity_group": 1},
            ],
        }),
    );
    fixtures.add(
        "board-join-across-categories",
        "A wand dropped on a +2 Ring of Might ×2 (#190 refused it): the ring keeps its stack as a member, its plain repeat now a bare ring copy under a label of its own — two Rings of Might, or the wand.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_might", "upgrade": 2},
                {"key": 2, "kind": "ring", "item": "ring_might"},
                {"key": 3, "kind": "wand"},
            ],
            "edits": [{"type": "join", "source": 3, "target": 1}],
        }),
    );
    fixtures.add(
        "board-join-refused",
        "Every stack label is taken, so a Wand of Disintegration dropped on Wand of Frost ×2 — whose stack would need a label of its own as a member — is refused, as the chip's refuse list says up front.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "upgrade": 1, "identity_group": 1},
                {"key": 2, "kind": "wand", "identity_group": 1},
                {"key": 3, "kind": "wand", "upgrade": 2, "identity_group": 2},
                {"key": 4, "kind": "wand", "identity_group": 2},
                {"key": 5, "kind": "armor", "upgrade": 1, "identity_group": 3},
                {"key": 6, "kind": "armor", "identity_group": 3},
                {"key": 7, "kind": "ring", "upgrade": 1, "identity_group": 4},
                {"key": 8, "kind": "ring", "identity_group": 4},
                {"key": 9, "kind": "wand", "item": "wand_frost"},
                {"key": 10, "kind": "wand", "item": "wand_frost"},
                {"key": 11, "kind": "wand", "item": "wand_disintegration"},
            ],
            "edits": [{"type": "join", "source": 11, "target": 9}],
        }),
    );
    fixtures.add(
        "board-join-leaves-copies",
        "A +2 Spear ×2 dragged onto a Tier 3 weapon: a drag carries a bare copy, so a plain Spear joins, and the +2 Spear stays behind on its own.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "weapon", "item": "spear", "upgrade": 2},
                {"key": 2, "kind": "weapon", "item": "spear"},
                {"key": 3, "kind": "weapon", "tier": {"exact": 3}},
            ],
            "edits": [{"type": "join", "source": 1, "target": 3}],
        }),
    );
    fixtures.add(
        "board-join-one-copy",
        "Wand of Disintegration ×2 dragged onto Wand of Frost: one Disintegration — the stack's last copy, key 20 — joins Frost, and the chip stays behind on its own (the stack's count never becomes the group's).",
        Board,
        &json!({
            "rows": repro(),
            "edits": [{"type": "join", "source": 1, "target": 7}],
        }),
    );
    fixtures.add(
        "board-join-round-trip",
        "The same join, then the joined Disintegration (key 20) detached: it folds back into the chip it came from, and Frost is alone again.",
        Board,
        &json!({
            "rows": repro(),
            "edits": [
                {"type": "join", "source": 1, "target": 7},
                {"type": "detach", "key": 20},
            ],
        }),
    );
    fixtures.add(
        "board-join-onto-stacked-chip",
        "Wand of Disintegration dropped on Wand of Frost ×2: Frost keeps its stack as a member, its plain repeat now a bare wand copy under a label only Frost carries — {Frost ×2 | Disintegration}, two Frosts or one Disintegration.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "item": "wand_frost"},
                {"key": 2, "kind": "wand", "item": "wand_frost"},
                {"key": 3, "kind": "wand", "item": "wand_disintegration"},
            ],
            "edits": [{"type": "join", "source": 3, "target": 1}],
        }),
    );
    fixtures.add(
        "board-join-onto-stacked-cluster",
        "Wand of Disintegration ×2 dropped on {Frost ×2 | Lightning ×2}: a bare Disintegration joins as a ×1 member, the members keep their shared stack, and the chip stays behind at ×1.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "item": "wand_frost", "alternative_group": 1, "identity_group": 1},
                {"key": 2, "kind": "wand", "item": "wand_lightning", "alternative_group": 1, "identity_group": 1},
                {"key": 3, "kind": "wand", "identity_group": 1},
                {"key": 4, "kind": "wand", "item": "wand_disintegration"},
                {"key": 5, "kind": "wand", "item": "wand_disintegration"},
            ],
            "edits": [{"type": "join", "source": 4, "target": 1}],
        }),
    );
    fixtures.add(
        "board-join-combined-level",
        "A Ring of Energy from three counting levels to at least 11 joins a Ring of Might, a plain ring: the two left behind keep counting, capped at the 8 they can reach.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_energy", "level_sum": {"group": 1, "at_least": 11}},
                {"key": 2, "kind": "ring", "item": "ring_energy", "level_sum": {"group": 1, "at_least": 11}},
                {"key": 3, "kind": "ring", "item": "ring_energy", "level_sum": {"group": 1, "at_least": 11}},
                {"key": 4, "kind": "ring", "item": "ring_might"},
            ],
            "edits": [{"type": "join", "source": 1, "target": 4}],
        }),
    );
    fixtures.add(
        "board-join-onto-combined-level",
        "A Ring of Might dropped on a Ring of Energy ×3 counting levels: the Energy stack keeps its count as a member's stack and drops the combined level, which cannot travel into a cluster.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_energy", "level_sum": {"group": 1, "at_least": 11}},
                {"key": 2, "kind": "ring", "item": "ring_energy", "level_sum": {"group": 1, "at_least": 11}},
                {"key": 3, "kind": "ring", "item": "ring_energy", "level_sum": {"group": 1, "at_least": 11}},
                {"key": 4, "kind": "ring", "item": "ring_might"},
            ],
            "edits": [{"type": "join", "source": 4, "target": 1}],
        }),
    );
    fixtures.add(
        "board-join-member-moves-one",
        "The +2 Frost of {Frost +2 ×2 | Disintegration ×2} dragged onto a +3 wand takes one item: a bare Frost joins the wand, the +2 Frost stays in the group at ×1, and Disintegration keeps its ×2.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "item": "wand_frost", "upgrade": 2, "alternative_group": 1, "identity_group": 1},
                {"key": 2, "kind": "wand", "item": "wand_disintegration", "alternative_group": 1, "identity_group": 1},
                {"key": 3, "kind": "wand", "identity_group": 1},
                {"key": 7, "kind": "wand", "upgrade": 3},
            ],
            "edits": [{"type": "join", "source": 1, "target": 7}],
        }),
    );
    fixtures.add(
        "board-join-hand-written-stack",
        "A Wand of Disintegration +3 ×3 written as bare copies under a stack label, the way a list from elsewhere may hold it, dragged onto Wand of Frost: a plain Disintegration joins, and the +3 chip keeps the other copy as a Disintegration, not Any wand.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "item": "wand_disintegration", "upgrade": 3, "identity_group": 1},
                {"key": 2, "kind": "wand", "identity_group": 1},
                {"key": 3, "kind": "wand", "identity_group": 1},
                {"key": 7, "kind": "wand", "item": "wand_frost"},
            ],
            "edits": [{"type": "join", "source": 1, "target": 7}],
        }),
    );
    fixtures.add(
        "board-detach-one-copy",
        "Frost out of {Frost ×2 | Disintegration}: a bare Frost leaves, on the key of the copy it was, and the member stays in the group at ×1 — {Frost | Disintegration} and Frost.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "item": "wand_frost", "alternative_group": 1, "identity_group": 1},
                {"key": 2, "kind": "wand", "item": "wand_disintegration", "alternative_group": 1},
                {"key": 3, "kind": "wand", "identity_group": 1},
            ],
            "edits": [{"type": "detach", "key": 1}],
        }),
    );
    fixtures.add(
        "board-join-bare-copy",
        "The reported drag: Ring of Energy +4 ×3 dropped on Wand of Disintegration. The item a drag carries is the chip's lifted face, a bare Ring of Energy — the copy remove_one takes — so {Disintegration | Ring of Energy} forms and the chip stays Ring of Energy +4 ×2.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_energy", "upgrade": 4},
                {"key": 2, "kind": "ring", "item": "ring_energy"},
                {"key": 3, "kind": "ring", "item": "ring_energy"},
                {"key": 4, "kind": "wand", "item": "wand_disintegration"},
            ],
            "edits": [{"type": "join", "source": 1, "target": 4}],
        }),
    );
    fixtures.add(
        "board-join-bare-copy-round-trip",
        "The same join, then the joined Ring of Energy (key 3) detached: it folds back into Ring of Energy +4 ×3, and Disintegration is alone again.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_energy", "upgrade": 4},
                {"key": 2, "kind": "ring", "item": "ring_energy"},
                {"key": 3, "kind": "ring", "item": "ring_energy"},
                {"key": 4, "kind": "wand", "item": "wand_disintegration"},
            ],
            "edits": [
                {"type": "join", "source": 1, "target": 4},
                {"type": "detach", "key": 3},
            ],
        }),
    );
    fixtures.add(
        "board-detach-bare-copy",
        "Frost +2 out of {Frost +2 ×2 | Disintegration}: a bare Wand of Frost leaves — the chip's lifted face — and the member keeps its +2 in the group at ×1: {Frost +2 | Disintegration} and Frost.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "item": "wand_frost", "upgrade": 2, "alternative_group": 1, "identity_group": 1},
                {"key": 2, "kind": "wand", "item": "wand_disintegration", "alternative_group": 1},
                {"key": 3, "kind": "wand", "identity_group": 1},
            ],
            "edits": [{"type": "detach", "key": 1}],
        }),
    );
    fixtures.add(
        "board-remove-one-member",
        "One item of the +2 Frost ×3 in {Frost +2 ×3 | Disintegration} dropped on the remove target: Frost steps down to ×2, keeping its upgrade and its label.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "item": "wand_frost", "upgrade": 2, "alternative_group": 1, "identity_group": 1},
                {"key": 2, "kind": "wand", "item": "wand_disintegration", "alternative_group": 1},
                {"key": 3, "kind": "wand", "identity_group": 1},
                {"key": 4, "kind": "wand", "identity_group": 1},
            ],
            "edits": [{"type": "remove_one", "key": 1}],
        }),
    );
    fixtures.add(
        "board-remove-one-stack",
        "One item of a Ring of Energy ×3 counting levels to at least 11 dropped on the remove target: two rings are left, their total capped at the 8 they can reach.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_energy", "level_sum": {"group": 1, "at_least": 11}},
                {"key": 2, "kind": "ring", "item": "ring_energy", "level_sum": {"group": 1, "at_least": 11}},
                {"key": 3, "kind": "ring", "item": "ring_energy", "level_sum": {"group": 1, "at_least": 11}},
            ],
            "edits": [{"type": "remove_one", "key": 1}],
        }),
    );
    fixtures.add(
        "board-lifted-never-normalized",
        "A list never normalized, drawn as written: Mace +1 alone beside a hand-written Mace ×2 (an anchor and a bare copy under a stack label). The edits run on the canonical rows, where that stack is two plain repeats of the Mace +1, so Mace +1 lifts a bare Mace although its copies are empty, and Mace ×2 moves itself (lifted null).",
        Board,
        &json!({
            "rows": [
                {"key": 3, "kind": "melee_weapon", "item": "mace", "upgrade": 1},
                {"key": 1, "kind": "melee_weapon", "item": "mace", "identity_group": 1},
                {"key": 2, "kind": "weapon", "identity_group": 1},
                {"key": 9, "kind": "wand", "item": "wand_frost"},
            ],
        }),
    );
    fixtures.add(
        "board-remove-one-as-drawn",
        "The bin on that lone Mace +1: a removal takes only what the board draws under the chip, so the hand-written Mace ×2 stays (as plain repeats once normalized) with Wand of Frost.",
        Board,
        &json!({
            "rows": [
                {"key": 3, "kind": "melee_weapon", "item": "mace", "upgrade": 1},
                {"key": 1, "kind": "melee_weapon", "item": "mace", "identity_group": 1},
                {"key": 2, "kind": "weapon", "identity_group": 1},
                {"key": 9, "kind": "wand", "item": "wand_frost"},
            ],
            "edits": [{"type": "remove_one", "key": 3}],
        }),
    );
    fixtures.add(
        "board-remaining-badges",
        "A Ring of Energy +4 ×3 at rest: a drag of it carries lifted, a bare Ring of Energy, while its origin shows remaining_badges, the +4 ×2 a remove_one would leave.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_energy", "upgrade": 4},
                {"key": 2, "kind": "ring", "item": "ring_energy"},
                {"key": 3, "kind": "ring", "item": "ring_energy"},
            ],
        }),
    );
    fixtures.add(
        "board-detach",
        "A cluster member taken out on its own; the cluster of one dissolves.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "item": "wand_fireblast", "alternative_group": 1},
                {"key": 2, "kind": "wand", "item": "wand_frost", "alternative_group": 1},
            ],
            "edits": [{"type": "detach", "key": 2}],
        }),
    );
    fixtures.add(
        "board-remove-member",
        "Removing one member of a three-way cluster keeps the other two together.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "item": "wand_fireblast", "alternative_group": 1},
                {"key": 2, "kind": "wand", "item": "wand_frost", "alternative_group": 1},
                {"key": 3, "kind": "wand", "item": "wand_lightning", "alternative_group": 1},
            ],
            "edits": [{"type": "remove", "key": 2}],
        }),
    );
    fixtures.add(
        "board-remove-item",
        "Removing a stack removes its hidden copies with it.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "upgrade": 3, "identity_group": 1},
                {"key": 2, "kind": "wand", "identity_group": 1},
                {"key": 3, "kind": "ring"},
            ],
            "edits": [{"type": "remove_item", "key": 1}],
        }),
    );
    fixtures.add(
        "board-copy-depth",
        "The copies of a stack limited to floor 10, which snaps down to floor 9 (floor 10 is an empty boss floor).",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_might", "upgrade": 2},
                {"key": 2, "kind": "ring", "item": "ring_might"},
            ],
            "edits": [{"type": "set_copy_depth", "key": 1, "max_depth": 10}],
        }),
    );
    fixtures.add(
        "board-toggle-levels",
        "Counting levels switched on for two rings starts at the default total.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_haste", "upgrade": 1},
                {"key": 2, "kind": "ring", "item": "ring_haste"},
            ],
            "edits": [{"type": "toggle_levels", "key": 1}],
        }),
    );
    fixtures.add(
        "board-toggle-levels-copy-floor",
        "Counting levels switched on for a Ring of Energy within floor 9 whose copy lies within floor 20: each keeps its own floor limit.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_energy", "max_depth": 9},
                {"key": 2, "kind": "ring", "item": "ring_energy", "max_depth": 20},
            ],
            "edits": [{"type": "toggle_levels", "key": 1}],
        }),
    );
    fixtures.add(
        "board-toggle-levels-off-copy-floor",
        "Counting levels switched off again: the copy is a plain repeat that keeps its floor 20.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_energy", "max_depth": 9,
                 "level_sum": {"group": 1, "at_least": 2}},
                {"key": 2, "kind": "ring", "item": "ring_energy", "max_depth": 20,
                 "level_sum": {"group": 1, "at_least": 2}},
            ],
            "edits": [{"type": "toggle_levels", "key": 1}],
        }),
    );
    fixtures.add(
        "board-stack-total-grow-copy-floor",
        "A counting Ring of Energy stack within floor 9 whose copy lies within floor 20, grown to three: the new copy takes the copies' floor 20.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_energy", "max_depth": 9,
                 "level_sum": {"group": 1, "at_least": 3}},
                {"key": 2, "kind": "ring", "item": "ring_energy", "max_depth": 20,
                 "level_sum": {"group": 1, "at_least": 3}},
            ],
            "edits": [{"type": "set_count", "key": 1, "count": 3}],
        }),
    );
    fixtures.add(
        "board-member-count-one",
        "{Frost ×2 | Disintegration ×2}, their alike stacks sharing a label, with Frost stepped down to ×1: Frost drops the label, and Disintegration keeps it with the copy.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "item": "wand_frost", "alternative_group": 1, "identity_group": 1},
                {"key": 2, "kind": "wand", "item": "wand_disintegration", "alternative_group": 1, "identity_group": 1},
                {"key": 3, "kind": "wand", "identity_group": 1},
            ],
            "edits": [{"type": "set_count", "key": 1, "count": 1}],
        }),
    );
    fixtures.add(
        "board-blanket-total-refused",
        "A blanket requirement cannot count levels together.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_might"},
                {"key": 2, "kind": "ring", "item": "ring_might", "blanket": true},
            ],
            "edits": [{"type": "toggle_levels", "key": 2}],
        }),
    );
    fixtures.add(
        "board-save-new",
        "A new chip saved from the sheet is appended with a fresh key.",
        Board,
        &json!({
            "rows": [{"key": 1, "kind": "wand"}],
            "next_key": 5,
            "edits": [{"type": "save", "key": null,
                       "requirement": {"kind": "thrown_weapon", "upgrade": {"at_least": 2}},
                       "count": 2, "total": null, "copy_depth": 6}],
        }),
    );
    fixtures.add(
        "board-save-unchanged",
        "Saving a chip exactly as it is changes nothing: the rows and their keys come back as they were.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_might", "upgrade": 2},
                {"key": 7, "kind": "ring", "item": "ring_might", "max_depth": 9},
            ],
            "edits": [{"type": "save", "key": 1,
                       "requirement": {"kind": "ring", "item": "ring_might", "upgrade": 2},
                       "count": 2, "total": null, "copy_depth": 9}],
        }),
    );
    fixtures.add(
        "board-problems",
        "Problems in order: each row's own in list order, then those between rows, blaming every row of the group.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "ring", "item": "ring_might", "level_sum": {"group": 1, "at_least": 3}},
                {"key": 2, "kind": "wand", "max_depth": 30},
                {"key": 3, "kind": "ring", "item": "ring_might", "level_sum": {"group": 1, "at_least": 4}},
                {"key": 4, "kind": "armor", "blanket": true, "identity_group": 2},
            ],
        }),
    );
    fixtures.add(
        "board-problems-blankets-only",
        "Blanket requirements alone constrain nothing: the list's own problem, blaming no row.",
        Board,
        &json!({"rows": [{"key": 1, "kind": "wand", "blanket": true}]}),
    );
    fixtures.add(
        "board-key-repair",
        "A zero key and a duplicate are re-keyed and reported; the edit's key follows the repair; wide alternative labels are compacted.",
        Board,
        &json!({
            "rows": [
                {"key": 0, "kind": "wand"},
                {"key": 4, "kind": "ring", "alternative_group": 300},
                {"key": 4, "kind": "armor", "alternative_group": 300},
            ],
            "edits": [{"type": "set_count", "key": 0, "count": 2}],
        }),
    );
    fixtures.add(
        "board-normalize-labels",
        "A hand-written stack labelled 7 and combined level labelled 9: normalizing moves both onto free labels in range, and their problems go.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "upgrade": 3, "identity_group": 7},
                {"key": 2, "kind": "wand", "identity_group": 7},
                {"key": 3, "kind": "ring", "item": "ring_might", "level_sum": {"group": 9, "at_least": 3}},
                {"key": 4, "kind": "ring", "item": "ring_might", "level_sum": {"group": 9, "at_least": 3}},
            ],
            "edits": [{"type": "normalize"}],
        }),
    );
    fixtures.add(
        "board-unreadable-row",
        "A row naming an item the catalog does not know is kept verbatim and shown as an unknown chip.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand"},
                {"key": 2, "kind": "wand", "item": "wand_of_wonders", "upgrade": 3},
                {"key": 3, "kind": "ring"},
            ],
        }),
    );
    fixtures.add(
        "board-unreadable-row-removed",
        "Removal is the one edit an unreadable row accepts; a join onto it does nothing.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand"},
                {"key": 2, "kind": "wand", "item": "wand_of_wonders", "upgrade": 3},
                {"key": 3, "kind": "ring"},
            ],
            "edits": [
                {"type": "join", "source": 1, "target": 2},
                {"type": "remove", "key": 2},
            ],
        }),
    );
    fixtures.add(
        "board-unreadable-row-labels",
        "The stack labels unreadable rows hold are never taken: Wand of Disintegration cannot join Frost ×2, which would need one as a member stack, and its chip refuses it with no_free_group.",
        Board,
        &json!({
            "rows": [
                {"key": 1, "kind": "wand", "item": "wand_frost"},
                {"key": 2, "kind": "wand", "item": "wand_frost"},
                {"key": 3, "kind": "wand", "item": "wand_disintegration"},
                {"key": 4, "kind": "ring", "item": "ring_of_wonders", "identity_group": 1},
                {"key": 5, "kind": "ring", "item": "ring_of_wonders", "identity_group": 2},
                {"key": 6, "kind": "ring", "item": "ring_of_wonders", "identity_group": 3},
                {"key": 7, "kind": "ring", "item": "ring_of_wonders", "identity_group": 4},
            ],
        }),
    );
    fixtures.add(
        "board-error-json",
        "A request that is not JSON.",
        Board,
        &Value::String("{\"rows\": [".to_owned()),
    );
    fixtures.add(
        "board-error-row",
        "A row without a key cannot be named, so the request fails.",
        Board,
        &json!({"rows": [{"kind": "wand"}]}),
    );
    fixtures.add(
        "board-error-edit",
        "An edit whose requirement cannot be read fails the request.",
        Board,
        &json!({
            "rows": [],
            "edits": [{"type": "save", "key": null, "requirement": {"item": "wand_of_wonders"},
                       "count": 1, "total": null, "copy_depth": null}],
        }),
    );
}

#[allow(clippy::too_many_lines)] // One pair after another, in reading order.
fn editor_fixtures(fixtures: &mut Fixtures) {
    use Envelope::Editor;
    let rows = json!([
        {"key": 1, "kind": "wand"},
        {"key": 2, "kind": "trinket", "item": "rat_skull"},
    ]);
    let opened = fixtures.add(
        "editor-open-new",
        "A sheet for a new chip starts on any weapon; Arcane Resin is offered among the wands.",
        Editor,
        &json!({"op": "open", "rows": rows, "key": null, "blanket": false, "resin": null,
               "offer_resin": true, "open_resin": false}),
    );
    let item = fixtures.add(
        "editor-change-item",
        "Picking a Spear names the item and drops the tier filter.",
        Editor,
        &json!({"op": "change", "draft": opened["draft"],
               "change": {"type": "set_item", "value": "spear"}}),
    );
    let effect = fixtures.add(
        "editor-change-effect",
        "Specific effects: ticking one enchantment.",
        Editor,
        &json!({"op": "change", "draft": fixtures_draft(&item, Editor,
                    &[json!({"type": "set_effect_mode", "value": "specific"})]),
               "change": {"type": "toggle_effect", "value": "Blazing"}}),
    );
    let stacked = fixtures.add(
        "editor-change-count",
        "Two Spears, the copies limited to floor 5, which steps on to floor 6 from floor 4.",
        Editor,
        &json!({"op": "change", "draft": fixtures_draft(&effect, Editor, &[
                    json!({"type": "set_count", "value": 2}),
                    json!({"type": "set_copy_depth_enabled", "value": true}),
               ]),
               "change": {"type": "set_copy_depth", "value": 5}}),
    );
    fixtures.add(
        "editor-save",
        "Saving the new chip appends it with its copy.",
        Editor,
        &json!({"op": "save", "draft": stacked["draft"], "rows": rows, "next_key": 3}),
    );
    let trinket = fixtures_draft(
        &opened,
        Editor,
        &[
            json!({"type": "set_category", "value": "trinket"}),
            json!({"type": "set_item", "value": "rat_skull"}),
        ],
    );
    fixtures.add(
        "editor-save-refused",
        "A second Rat Skull cannot be saved: each trinket appears once in the deck.",
        Editor,
        &json!({"op": "save", "draft": trinket, "rows": rows}),
    );
    fixtures.add(
        "editor-open-row",
        "A sheet on a ring stack counting levels.",
        Editor,
        &json!({"op": "open", "key": 1, "rows": [
            {"key": 1, "kind": "ring", "item": "ring_might", "level_sum": {"group": 1, "at_least": 3}},
            {"key": 2, "kind": "ring", "item": "ring_might", "level_sum": {"group": 1, "at_least": 3}},
        ]}),
    );
    let member_rows = json!([
        {"key": 1, "kind": "wand", "item": "wand_frost", "alternative_group": 1, "identity_group": 1},
        {"key": 2, "kind": "wand", "item": "wand_disintegration", "alternative_group": 1},
        {"key": 3, "kind": "wand", "identity_group": 1},
    ]);
    let member = fixtures.add(
        "editor-open-member",
        "A sheet on the Frost of {Frost ×2 | Disintegration}: a member's stack is its own, so the sheet shows its count and copy floor, but no combined level.",
        Editor,
        &json!({"op": "open", "key": 1, "rows": member_rows}),
    );
    fixtures.add(
        "editor-save-member",
        "The member saved at ×3: Frost's own stack grows by a copy, and Disintegration stays ×1.",
        Editor,
        &json!({"op": "save",
               "draft": fixtures_draft(&member, Editor, &[json!({"type": "set_count", "value": 3})]),
               "rows": member_rows}),
    );
    fixtures.add(
        "editor-open-blanket",
        "A new blanket starts on the kind of the first ordinary row, melee narrowing included.",
        Editor,
        &json!({"op": "open", "blanket": true, "rows": [
            {"key": 1, "kind": "melee_weapon", "tier": {"exact": 4}},
        ]}),
    );
    let wand = fixtures.add(
        "editor-resin-pick",
        "A wand chip switched to Arcane Resin: the sheet edits the query's resin, seeded from it.",
        Editor,
        &json!({"op": "change",
               "draft": fixtures_draft(&fixtures_open(Editor, &json!({
                   "op": "open", "rows": rows, "key": 1, "offer_resin": true,
                   "resin": {"amount": 4, "filter": {"uncursed": true, "max_depth": 14,
                             "source": null, "include_mage_wand": false}},
               })), Editor, &[]),
               "change": {"type": "set_item", "value": "arcane_resin"}}),
    );
    fixtures.add(
        "editor-resin-save-set",
        "Saving Arcane Resin sets the query's resin and removes the wand chip the sheet was opened on.",
        Editor,
        &json!({"op": "save", "draft": wand["draft"], "rows": rows}),
    );
    let resin = fixtures.add(
        "editor-resin-open",
        "The resin chip opens as Arcane Resin on an Auto amount.",
        Editor,
        &json!({"op": "open", "rows": rows, "open_resin": true, "offer_resin": true,
               "resin": {"amount": "auto", "filter": {"uncursed": false, "max_depth": null,
                         "source": "chest", "include_mage_wand": true}}}),
    );
    let frost = fixtures_draft(
        &resin,
        Editor,
        &[json!({"type": "set_item", "value": "wand_frost"})],
    );
    fixtures.add(
        "editor-resin-save-clear",
        "The resin chip saved as a Wand of Frost adds the wand and clears the query's resin.",
        Editor,
        &json!({"op": "save", "draft": frost, "rows": rows}),
    );
    fixtures.add(
        "editor-resin-amount-invalid",
        "An empty amount field keeps the sheet from saving.",
        Editor,
        &json!({"op": "change",
               "draft": fixtures_draft(&resin, Editor,
                    &[json!({"type": "set_resin_auto", "value": false})]),
               "change": {"type": "set_resin_amount", "value": null}}),
    );
    let sandals = json!([
        {"key": 1, "kind": "artifact", "item": "sandals_of_nature", "upgrade": 5, "max_depth": 19},
    ]);
    let untouched = fixtures_open(Editor, &json!({"op": "open", "rows": sandals, "key": 1}));
    fixtures.add(
        "editor-save-untouched",
        "An artifact asking for the city vault's +5, which the sheet has no control for, saved untouched: the rows come back as they were.",
        Editor,
        &json!({"op": "save", "draft": untouched["draft"], "rows": sandals}),
    );
    let wrong = json!([
        {"key": 1, "kind": "ring"},
        {"key": 2, "kind": "wand", "blanket": true, "exclude_resin": true},
    ]);
    let repaired = fixtures_open(Editor, &json!({"op": "open", "rows": wrong, "key": 2}));
    fixtures.add(
        "editor-save-untouched-repairs",
        "A hand-written blanket wand excluded from Auto resin, which only an ordinary wand can be: the sheet opens without the exclusion, and saved untouched it writes the repaired row.",
        Editor,
        &json!({"op": "save", "draft": repaired["draft"], "rows": wrong}),
    );
    let resin_rows = json!([{"key": 1, "kind": "wand", "item": "wand_frost"}]);
    let kept = fixtures_open(
        Editor,
        &json!({"op": "open", "rows": resin_rows, "open_resin": true,
               "resin": {"amount": 4, "filter": {"uncursed": true, "max_depth": 5,
                         "source": "locked_chest", "include_mage_wand": true}}}),
    );
    fixtures.add(
        "editor-resin-save-untouched",
        "The resin chip saved untouched leaves the query's resin as it is (resin null), floor 5 included, which the floor slider shows as 4.",
        Editor,
        &json!({"op": "save", "draft": kept["draft"], "rows": resin_rows}),
    );
    let added = fixtures.add(
        "editor-resin-open-new",
        "A resin sheet on a query without resin adds one: a new sheet with Arcane Resin picked.",
        Editor,
        &json!({"op": "open", "rows": [], "open_resin": true}),
    );
    fixtures.add(
        "editor-resin-mage-wand",
        "The Mage's starting wand counted in the resin section, which words its choice, its amount's bounds and its switch.",
        Editor,
        &json!({"op": "change", "draft": added["draft"],
               "change": {"type": "set_include_mage_wand", "value": true}}),
    );
    fixtures.add(
        "editor-error-draft",
        "A draft from another version (or none at all) cannot be read; the platform reopens the sheet.",
        Editor,
        &json!({"op": "change", "draft": "{\"v\":99}",
               "change": {"type": "set_uncursed", "value": true}}),
    );
    fixtures.add(
        "editor-error-unreadable-row",
        "An unreadable row has no sheet; the error names its key.",
        Editor,
        &json!({"op": "open", "key": 2, "rows": [
            {"key": 1, "kind": "wand"},
            {"key": 2, "kind": "wand", "item": "wand_of_wonders"},
        ]}),
    );
}

/// A sheet response's draft after `changes`, applied through the envelope
/// without recording them.
fn fixtures_draft(response: &Value, envelope: Envelope, changes: &[Value]) -> Value {
    changes
        .iter()
        .fold(response["draft"].clone(), |draft, change| {
            let next = envelope.call(&json!({"op": "change", "draft": draft, "change": change}));
            assert!(next.get("error").is_none(), "{change}: {next}");
            next["draft"].clone()
        })
}

/// A sheet opened without recording it.
fn fixtures_open(envelope: Envelope, request: &Value) -> Value {
    let response = envelope.call(request);
    assert!(response.get("error").is_none(), "{request}: {response}");
    response
}

fn directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor")
}

fn render(fixture: &Value) -> String {
    let mut text = serde_json::to_string_pretty(fixture).expect("fixtures serialize");
    text.push('\n');
    text
}

#[test]
fn the_envelopes_answer_the_golden_fixtures() {
    let mut fixtures = Fixtures::default();
    board_fixtures(&mut fixtures);
    editor_fixtures(&mut fixtures);
    let directory = directory();
    let names: BTreeSet<&str> = fixtures
        .files
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(
        names.len(),
        fixtures.files.len(),
        "fixture names are unique"
    );

    if std::env::var_os("UPDATE_EDITOR_FIXTURES").is_some_and(|value| value == "1") {
        fs::create_dir_all(&directory).expect("create the fixture directory");
        for entry in fs::read_dir(&directory).expect("list the fixtures") {
            let path = entry.expect("a fixture entry").path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                fs::remove_file(path).expect("remove a stale fixture");
            }
        }
        for (name, fixture) in &fixtures.files {
            fs::write(directory.join(name), render(fixture)).expect("write a fixture");
        }
        return;
    }

    for (name, fixture) in &fixtures.files {
        let path = directory.join(name);
        let stored = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "{}: {error}; regenerate with UPDATE_EDITOR_FIXTURES=1",
                path.display()
            )
        });
        // Line endings a checkout converted (`core.autocrlf` on Windows) are
        // not drift; `.gitattributes` also keeps these files LF.
        assert!(
            stored.replace("\r\n", "\n") == render(fixture),
            "{name} drifted from what the envelopes answer; review the change and regenerate \
             with UPDATE_EDITOR_FIXTURES=1"
        );
    }
    let stored: BTreeSet<String> = fs::read_dir(&directory)
        .expect("list the fixtures")
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            Path::new(&name)
                .extension()
                .is_some_and(|extension| extension == "json")
                .then_some(name)
        })
        .collect();
    let expected: BTreeSet<String> = names.iter().map(|name| (*name).to_owned()).collect();
    assert_eq!(
        stored, expected,
        "stale fixtures; regenerate with UPDATE_EDITOR_FIXTURES=1"
    );
}

/// Every row the fixtures send — hand-written lists, key repairs, problems
/// and all — opens into a sheet that, saved untouched onto the same rows,
/// answers what the board answers for those rows alone: a sheet no one
/// changed writes nothing of its own — unless it repairs the list, taking
/// a problem away and adding none. Rows that cannot be opened (an
/// unreadable one) are left out, and the one refusal an untouched sheet may
/// meet is a duplicate trinket the list already holds.
#[test]
fn every_fixture_row_saves_back_untouched() {
    if std::env::var_os("UPDATE_EDITOR_FIXTURES").is_some_and(|value| value == "1") {
        return;
    }
    let mut lists = BTreeSet::new();
    for entry in fs::read_dir(directory()).expect("list the fixtures") {
        let path = entry.expect("a fixture entry").path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let fixture: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read a fixture"))
                .expect("a fixture is JSON");
        if let Some(rows) = fixture["request"]
            .get("rows")
            .filter(|rows| rows.is_array())
        {
            lists.insert(rows.to_string());
        }
    }
    let (mut saved, mut repaired) = (0, 0);
    for list in &lists {
        let rows: Value = serde_json::from_str(list).expect("rows are JSON");
        let board = Envelope::Board.call(&json!({"rows": rows}));
        if board.get("error").is_some() {
            continue;
        }
        for key in rows
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|row| row["key"].as_u64())
        {
            let opened = Envelope::Editor.call(&json!({"op": "open", "rows": rows, "key": key}));
            if opened.get("error").is_some() {
                continue;
            }
            let answer = Envelope::Editor
                .call(&json!({"op": "save", "draft": opened["draft"], "rows": rows}));
            let context = format!("{list} at {key}: {answer}");
            match answer.get("saved") {
                Some(result) if result["rows"] != board["rows"] => {
                    let after = Envelope::Board.call(&json!({"rows": result["rows"]}));
                    let problems = |board: &Value| -> BTreeSet<String> {
                        board["problems"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(Value::to_string)
                            .collect()
                    };
                    let (before, after) = (problems(&board), problems(&after));
                    assert!(after.is_subset(&before) && after != before, "{context}");
                    assert_eq!(result["changed"], json!(true), "{context}");
                    repaired += 1;
                }
                Some(result) => {
                    assert_eq!(result["changed"], board["changed"], "{context}");
                    assert_eq!(result["resin"], Value::Null, "{context}");
                    saved += 1;
                }
                None => assert_eq!(
                    answer["form"]["errors"],
                    json!([
                        "This trinket is already required. Each trinket appears only once in the deck."
                    ]),
                    "{context}"
                ),
            }
        }
    }
    assert!(saved > 0 && repaired > 0);
}

/// The files replay: every stored request, sent as a platform would send
/// it, answers its stored response. (A platform test does the same through
/// its own binding.)
#[test]
fn every_stored_request_answers_its_stored_response() {
    if std::env::var_os("UPDATE_EDITOR_FIXTURES").is_some_and(|value| value == "1") {
        // The files are being rewritten alongside.
        return;
    }
    let Ok(entries) = fs::read_dir(directory()) else {
        panic!("no fixtures; generate them with UPDATE_EDITOR_FIXTURES=1");
    };
    let mut replayed = 0;
    for entry in entries {
        let path = entry.expect("a fixture entry").path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let fixture: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read a fixture"))
                .expect("a fixture is JSON");
        let envelope = match fixture["envelope"].as_str() {
            Some("requirement_board") => Envelope::Board,
            Some("requirement_editor") => Envelope::Editor,
            other => panic!("{}: unknown envelope {other:?}", path.display()),
        };
        let response = envelope.call(&fixture["request"]);
        assert_eq!(response, fixture["response"], "{}", path.display());
        replayed += 1;
    }
    assert!(replayed > 0);
}
