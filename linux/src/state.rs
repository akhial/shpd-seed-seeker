// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared query state and presentation labels for the whole window.

use shpd_seedfinder_core::catalog::{ItemKind, WeaponCategory};
use shpd_seedfinder_core::challenges::Challenges;
use shpd_seedfinder_core::editor::{
    self, Draft, Edit, EditResult, ResinAmount, ResinOutcome, ResinState, Row, SaveResult,
};
use shpd_seedfinder_core::feasibility::Quest;
use shpd_seedfinder_core::model::ItemSource;
use shpd_seedfinder_core::query::{ArcaneResinFilter, SearchQuery};
use shpd_seedfinder_core::quests::{
    BlacksmithQuestType, GhostQuestType, ImpQuestType, QuestSummary, WandmakerQuestType,
};

use shpd_seedfinder_core::floor_filters::{FloorRequirement, RoomType};
use shpd_seedfinder_core::level_prelude::Feeling;

pub const FARMING_FLOORS: [u8; 3] = [7, 17, 22];

pub fn is_farming_requirement(floor: &FloorRequirement) -> bool {
    FARMING_FLOORS.contains(&floor.depth)
        && floor.feeling == Some(Feeling::Dark)
        && floor.rooms.is_empty()
        && floor.any_rooms.len() == 2
        && floor.any_rooms.contains(&RoomType::SpecialGarden)
        && floor.any_rooms.contains(&RoomType::SecretGarden)
}

/// One entry in the requirement editor's category picker: an item family,
/// optionally narrowed to one weapon class.
pub type KindChoice = (ItemKind, Option<WeaponCategory>);

/// Every user-facing category choice, in presentation order. A plain weapon
/// requirement keeps matching melee and thrown weapons alike.
pub const ALL_KIND_CHOICES: &[KindChoice] = &[
    (ItemKind::Weapon, None),
    (ItemKind::Weapon, Some(WeaponCategory::Melee)),
    (ItemKind::Weapon, Some(WeaponCategory::Thrown)),
    (ItemKind::Armor, None),
    (ItemKind::Wand, None),
    (ItemKind::Ring, None),
    (ItemKind::Trinket, None),
    (ItemKind::Artifact, None),
];

/// The whole persisted query state shared by all panes.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // Mirrors independent engine query options.
pub struct AppState {
    pub arcane_resin: u16,
    pub arcane_resin_auto: bool,
    pub arcane_resin_filter: ArcaneResinFilter,
    pub auto_apply_trinket: bool,
    pub floor_requirements: Vec<FloorRequirement>,
    /// Both board sections in one list, each row under a session-stable key;
    /// the shared editor folds them into the board and edits them.
    pub requirements: Vec<Row>,
    pub max_depth: u8,
    pub require_blacksmith: bool,
    pub exclude_blacksmith_rewards: bool,
    pub wandmaker_quest: Option<WandmakerQuestType>,
    pub challenges: Challenges,
    next_key: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            auto_apply_trinket: true,
            arcane_resin: 0,
            arcane_resin_auto: false,
            arcane_resin_filter: ArcaneResinFilter::default(),
            floor_requirements: Vec::new(),
            requirements: Vec::new(),
            max_depth: 24,
            require_blacksmith: false,
            exclude_blacksmith_rewards: false,
            wandmaker_quest: None,
            challenges: Challenges::NONE,
            next_key: 1,
        }
    }
}

impl AppState {
    pub fn toggle_farming_floor(&mut self, depth: u8) {
        assert!(FARMING_FLOORS.contains(&depth));
        let selected = self
            .floor_requirements
            .iter()
            .any(|floor| floor.depth == depth && is_farming_requirement(floor));
        self.floor_requirements.retain(|floor| floor.depth != depth);
        if !selected {
            self.floor_requirements.push(FloorRequirement {
                depth,
                feeling: Some(Feeling::Dark),
                rooms: Vec::new(),
                any_rooms: vec![RoomType::SpecialGarden, RoomType::SecretGarden],
            });
            self.max_depth = self.max_depth.max(depth);
        }
        self.floor_requirements.sort_by_key(|floor| floor.depth);
    }

    pub const fn needs_resin(&self) -> bool {
        self.arcane_resin_auto || self.arcane_resin > 0
    }

    /// The query's Arcane Resin condition as the editor reads it — for the
    /// resin chip and the sheet's resin section — or `None` when the query
    /// asks for no resin.
    #[must_use]
    pub const fn resin(&self) -> Option<ResinState> {
        if !self.needs_resin() {
            return None;
        }
        Some(ResinState {
            amount: if self.arcane_resin_auto {
                ResinAmount::Auto
            } else {
                ResinAmount::AtLeast(self.arcane_resin)
            },
            filter: self.arcane_resin_filter,
        })
    }

    /// Sets the query's Arcane Resin condition, or with `None` drops it.
    pub fn set_resin(&mut self, resin: Option<ResinState>) {
        let Some(resin) = resin else {
            self.arcane_resin = 0;
            self.arcane_resin_auto = false;
            self.arcane_resin_filter = ArcaneResinFilter::default();
            return;
        };
        (self.arcane_resin_auto, self.arcane_resin) = match resin.amount {
            ResinAmount::Auto => (true, 0),
            ResinAmount::AtLeast(amount) => (false, amount),
        };
        self.arcane_resin_filter = resin.filter;
    }

    /// Hands out a fresh row key, unique within this session.
    pub const fn claim_key(&mut self) -> u64 {
        let key = self.next_key;
        self.next_key += 1;
        key
    }

    /// Rebuilds editor state from a decoded engine query, keying its rows
    /// 1…n. The rows are kept exactly as the query holds them; a query the
    /// user loads into the editor goes through [`Self::load`] instead.
    #[must_use]
    pub fn from_query(query: &SearchQuery) -> Self {
        let mut state = Self {
            auto_apply_trinket: query.auto_apply_trinket,
            arcane_resin: query.arcane_resin,
            arcane_resin_auto: query.arcane_resin_auto,
            arcane_resin_filter: query.arcane_resin_filter,
            floor_requirements: query.floor_requirements.clone(),
            requirements: Vec::with_capacity(query.requirements.len()),
            max_depth: query.max_depth,
            require_blacksmith: query.require_blacksmith,
            exclude_blacksmith_rewards: query.exclude_blacksmith_rewards,
            wandmaker_quest: query.wandmaker_quest,
            challenges: query.challenges,
            next_key: 1,
        };
        for requirement in &query.requirements {
            let key = state.claim_key();
            state.requirements.push(Row {
                key,
                requirement: *requirement,
            });
        }
        state
    }

    /// Rebuilds editor state from a query the user loaded or imported — a
    /// share link, a results file — keyed 1…n and brought once into the
    /// editor's canonical encoding, so the board writes back what it reads.
    #[must_use]
    pub fn load(query: &SearchQuery) -> Self {
        let mut state = Self::from_query(query);
        state.normalize();
        state
    }

    /// Rewrites the rows into the editor's canonical encoding: groups that
    /// no longer say anything dissolve, and a stack of one named item is
    /// written as plain repeats. A canonical list stays exactly as it was.
    pub fn normalize(&mut self) {
        self.apply(&[Edit::Normalize]);
    }

    /// Runs board edits through the shared editor. The rows are written back
    /// only when the editor changed them, so an edit that says nothing leaves
    /// the query — and a search resumed on it — exactly as it was; the key
    /// counter only ever moves forward.
    pub fn apply(&mut self, edits: &[Edit]) -> EditResult {
        let result = editor::apply(&self.requirements, Some(self.next_key), edits);
        self.adopt(&result);
        result
    }

    /// Opens the requirement sheet on the visible row `key`, or — with
    /// `None` — on a new chip of the ordinary or blanket section under a key
    /// claimed now, which a cancelled sheet simply leaves unused.
    pub fn open_sheet(&mut self, key: Option<u64>, blanket: bool) -> Draft {
        let key = key.unwrap_or_else(|| self.claim_key());
        let resin = self.resin();
        editor::open(
            &self.requirements,
            Some(key),
            blanket,
            resin.as_ref(),
            false,
            false,
        )
    }

    /// Saves a requirement sheet onto the rows as they are now — they may
    /// have moved while it was open — together with what the sheet made of
    /// the query's Arcane Resin.
    ///
    /// # Errors
    ///
    /// Returns why the editor refused the draft, in the order to show the
    /// reasons; nothing is stored then.
    pub fn save(&mut self, draft: &Draft) -> Result<EditResult, Vec<String>> {
        match editor::save(draft, &self.requirements, Some(self.next_key)) {
            SaveResult::Saved { result, resin } => {
                self.adopt(&result);
                match resin {
                    ResinOutcome::Unchanged => {}
                    ResinOutcome::Set(resin) => self.set_resin(Some(resin)),
                    ResinOutcome::Clear => self.set_resin(None),
                }
                Ok(result)
            }
            SaveResult::Refused { form, .. } => Err(form.errors),
        }
    }

    /// Takes an editor answer's rows, when they changed, and its key counter.
    fn adopt(&mut self, result: &EditResult) {
        if result.changed {
            self.requirements.clone_from(&result.rows);
        }
        self.next_key = self.next_key.max(result.next_key);
    }

    /// The state as an engine query exactly as the user left it, without
    /// checking that it is a runnable search: persistence and the seed
    /// scout read half-finished queries too.
    #[must_use]
    pub fn unvalidated_query(&self) -> SearchQuery {
        SearchQuery {
            floor_requirements: self.floor_requirements.clone(),
            auto_apply_trinket: self.auto_apply_trinket,
            arcane_resin_filter: self.arcane_resin_filter,
            arcane_resin_auto: self.arcane_resin_auto,
            arcane_resin: self.arcane_resin,
            requirements: self
                .requirements
                .iter()
                .map(|row| row.requirement)
                .collect(),
            max_depth: self.max_depth,
            challenges: self.challenges,
            require_blacksmith: self.require_blacksmith,
            exclude_blacksmith_rewards: self.exclude_blacksmith_rewards,
            wandmaker_quest: self.wandmaker_quest,
        }
    }

    /// Builds the validated engine query for the current state.
    ///
    /// # Errors
    ///
    /// Returns the human-readable validation message.
    pub fn to_query(&self) -> Result<SearchQuery, String> {
        let mut query = self.unvalidated_query();
        // Past the last floor the Blacksmith can first appear on the
        // quest is certain, so the filter would exclude nothing.
        query.require_blacksmith =
            self.require_blacksmith && self.max_depth < Quest::Blacksmith.window().1;
        query.validate().map_err(|error| error.to_string())?;
        Ok(query)
    }
}

pub const fn kind_choice_label(choice: KindChoice) -> &'static str {
    match choice {
        (ItemKind::Weapon, None) => "Weapon",
        (ItemKind::Weapon, Some(WeaponCategory::Melee)) => "Melee weapon",
        (ItemKind::Weapon, Some(WeaponCategory::Thrown)) => "Thrown weapon",
        (ItemKind::Armor, _) => "Armor",
        (ItemKind::Wand, _) => "Wand",
        (ItemKind::Ring, _) => "Ring",
        (ItemKind::Trinket, _) => "Trinket",
        (ItemKind::Artifact, _) => "Artifact",
    }
}

pub const fn kind_choice_singular(choice: KindChoice) -> &'static str {
    match choice {
        (ItemKind::Weapon, None) => "weapon",
        (ItemKind::Weapon, Some(WeaponCategory::Melee)) => "melee weapon",
        (ItemKind::Weapon, Some(WeaponCategory::Thrown)) => "thrown weapon",
        (ItemKind::Armor, _) => "armor",
        (ItemKind::Wand, _) => "wand",
        (ItemKind::Ring, _) => "ring",
        (ItemKind::Trinket, _) => "trinket",
        (ItemKind::Artifact, _) => "artifact",
    }
}

/// Bundled symbolic icon name for one item family, with a dedicated glyph
/// for thrown weapons.
pub const fn kind_icon(kind: ItemKind, weapon_category: Option<WeaponCategory>) -> &'static str {
    match (kind, weapon_category) {
        (ItemKind::Weapon, Some(WeaponCategory::Thrown)) => "kind-weapon-thrown-symbolic",
        (ItemKind::Weapon, _) => "kind-weapon-symbolic",
        (ItemKind::Armor, _) => "kind-armor-symbolic",
        (ItemKind::Wand, _) => "kind-wand-symbolic",
        (ItemKind::Ring, _) => "kind-ring-symbolic",
        (ItemKind::Trinket | ItemKind::Artifact, _) => "starred-symbolic",
    }
}

pub const fn source_label(source: ItemSource) -> &'static str {
    match source {
        ItemSource::Heap => "Floor",
        ItemSource::Chest => "Chest",
        ItemSource::LockedChest => "Locked chest",
        ItemSource::CrystalChest => "Crystal chest",
        ItemSource::Tomb => "Tomb",
        ItemSource::Skeleton => "Skeletal remains",
        ItemSource::SacrificialFire => "Sacrificial fire",
        ItemSource::Mimic => "Mimic",
        ItemSource::GoldenMimic => "Golden mimic",
        ItemSource::CrystalMimic => "Crystal mimic",
        ItemSource::Statue => "Animated statue",
        ItemSource::ArmoredStatue => "Armored statue",
        ItemSource::Shop => "Shop",
        ItemSource::GhostReward => "Sad ghost reward",
        ItemSource::WandmakerReward => "Wandmaker reward",
        ItemSource::BlacksmithReward => "Blacksmith reward",
        ItemSource::ImpReward => "Imp reward",
        ItemSource::VaultTreasure => "Vault treasure",
    }
}

/// Dungeon region name for one depth.
pub const fn region(depth: u8) -> &'static str {
    match depth {
        0..=5 => "Sewers",
        6..=10 => "Prison",
        11..=15 => "Caves",
        16..=20 => "Dwarven City",
        _ => "Demon Halls",
    }
}

pub const fn ghost_quest_label(variant: GhostQuestType) -> &'static str {
    match variant {
        GhostQuestType::FetidRat => "Fetid Rat",
        GhostQuestType::GnollTrickster => "Gnoll Trickster",
        GhostQuestType::GreatCrab => "Great Crab",
    }
}

pub const fn wandmaker_quest_label(variant: WandmakerQuestType) -> &'static str {
    match variant {
        WandmakerQuestType::CorpseDust => "Corpse Dust",
        WandmakerQuestType::ElementalEmbers => "Elemental Embers",
        WandmakerQuestType::Rotberry => "Rotberry",
    }
}

pub const fn blacksmith_quest_label(variant: BlacksmithQuestType) -> &'static str {
    match variant {
        BlacksmithQuestType::Crystal => "Crystal Spire",
        BlacksmithQuestType::Gnoll => "Gnoll Geomancer",
    }
}

pub const fn imp_target_label(variant: ImpQuestType) -> &'static str {
    match variant {
        ImpQuestType::Vault => "Vault",
    }
}

/// One scheduled quest prepared for presentation: the giver's name, the rolled
/// variant's label, and the giver's floor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuestRow {
    pub giver: &'static str,
    pub variant: &'static str,
    pub depth: u8,
}

/// The quests scheduled in one world, in dungeon order.
#[must_use]
pub fn quest_rows(quests: QuestSummary) -> Vec<QuestRow> {
    let mut rows = Vec::with_capacity(4);
    if let Some(quest) = quests.ghost {
        rows.push(QuestRow {
            giver: "Sad Ghost",
            variant: ghost_quest_label(quest.variant),
            depth: quest.depth,
        });
    }
    if let Some(quest) = quests.wandmaker {
        rows.push(QuestRow {
            giver: "Wandmaker",
            variant: wandmaker_quest_label(quest.variant),
            depth: quest.depth,
        });
    }
    if let Some(quest) = quests.blacksmith {
        rows.push(QuestRow {
            giver: "Blacksmith",
            variant: blacksmith_quest_label(quest.variant),
            depth: quest.depth,
        });
    }
    if let Some(quest) = quests.imp {
        rows.push(QuestRow {
            giver: "Imp",
            variant: imp_target_label(quest.variant),
            depth: quest.depth,
        });
    }
    rows
}

/// One upstream challenge with presentation data.
pub struct ChallengeInfo {
    pub challenge: Challenges,
    pub label: &'static str,
}

/// The nine upstream challenges, in mask order.
pub const ALL_CHALLENGES: &[ChallengeInfo] = &[
    ChallengeInfo {
        challenge: Challenges::NO_FOOD,
        label: "On diet",
    },
    ChallengeInfo {
        challenge: Challenges::NO_ARMOR,
        label: "Faith is my armor",
    },
    ChallengeInfo {
        challenge: Challenges::NO_HEALING,
        label: "Pharmacophobia",
    },
    ChallengeInfo {
        challenge: Challenges::NO_HERBALISM,
        label: "Barren land",
    },
    ChallengeInfo {
        challenge: Challenges::SWARM_INTELLIGENCE,
        label: "Swarm intelligence",
    },
    ChallengeInfo {
        challenge: Challenges::DARKNESS,
        label: "Into darkness",
    },
    ChallengeInfo {
        challenge: Challenges::NO_SCROLLS,
        label: "Forbidden runes",
    },
    ChallengeInfo {
        challenge: Challenges::CHAMPION_ENEMIES,
        label: "Hostile champions",
    },
    ChallengeInfo {
        challenge: Challenges::STRONGER_BOSSES,
        label: "Badder bosses",
    },
];

#[cfg(test)]
mod tests {
    use shpd_seedfinder_core::catalog::{ItemId, ItemKind};
    use shpd_seedfinder_core::editor::{self, BoardView, Draft, Edit, ItemView, Row};
    use shpd_seedfinder_core::query::{Requirement, TierRequirement, UpgradeRequirement};
    use shpd_seedfinder_core::quests::{
        BlacksmithQuestType, GhostQuestType, ImpQuestType, QuestSummary, ScheduledQuest,
        WandmakerQuestType,
    };

    use super::{
        AppState, QuestRow, blacksmith_quest_label, ghost_quest_label, imp_target_label,
        quest_rows, source_label, wandmaker_quest_label,
    };

    fn row(key: u64, requirement: Requirement) -> Row {
        Row { key, requirement }
    }

    fn board(state: &AppState) -> BoardView {
        editor::board_view(&state.requirements, state.resin().as_ref())
    }

    /// The board entry showing the row `key`.
    fn entry(state: &AppState, key: u64) -> ItemView {
        board(state)
            .items
            .into_iter()
            .find(|item| item.members.contains(&key))
            .expect("the row is on the board")
    }

    /// A sheet opened on `key` (or a new chip) and filled in the way the
    /// requirement dialog fills it: the requirement its controls describe,
    /// with the stack it asks for.
    fn sheet(
        state: &mut AppState,
        key: Option<u64>,
        requirement: Requirement,
        (count, total, copy_depth): (u8, Option<u8>, Option<u8>),
    ) -> Draft {
        let opened = state.open_sheet(key, requirement.blanket);
        Draft {
            requirement,
            count,
            total,
            copy_depth,
            ..opened
        }
    }

    #[test]
    fn blankets_survive_queries_links_and_persistence() {
        let query = shpd_seedfinder_core::json_query::decode(
            r#"{"requirements":[
            {"item":"wand_lightning","upgrade":{"at_least":2}},
            {"item":"wand_disintegration","upgrade":{"at_least":2}},
            {"item":"wand_frost","upgrade":{"at_least":2}},
            {"kind":"wand","upgrade":3,"source":"wandmaker_reward","blanket":true}
        ]}"#,
        )
        .unwrap();
        let state = AppState::from_query(&query);
        assert_eq!(
            state
                .requirements
                .iter()
                .map(|row| row.requirement.blanket)
                .collect::<Vec<_>>(),
            [false, false, false, true]
        );
        assert_eq!(state.to_query().unwrap(), query);
        let link = shpd_seedfinder_core::deep_link::encode(&query).unwrap();
        let decoded = shpd_seedfinder_core::deep_link::decode(&link).unwrap();
        assert_eq!(AppState::from_query(&decoded).to_query().unwrap(), query);
    }

    #[test]
    fn blankets_stay_separate_from_stacks_and_ordinary_alternatives() {
        let query = shpd_seedfinder_core::json_query::decode(
            r#"{"requirements":[
            {"item":"wand_frost"}, {"item":"wand_frost","blanket":true},
            {"item":"wand_frost","blanket":true}
        ]}"#,
        )
        .unwrap();
        let mut state = AppState::load(&query);
        assert_eq!(board(&state).items.len(), 3);
        assert!(!entry(&state, 2).stack.can_grow);
        assert!(!state.apply(&[Edit::SetCount { key: 2, count: 3 }]).changed);
        assert_eq!(state.requirements.len(), 3);
        // An ordinary chip never joins a blanket.
        assert!(
            !state
                .apply(&[Edit::Join {
                    source: 1,
                    target: 2
                }])
                .changed
        );
        assert_eq!(board(&state).items.len(), 3);
        assert!(
            state
                .apply(&[Edit::Join {
                    source: 2,
                    target: 3
                }])
                .changed
        );
        assert_eq!(board(&state).counts.blanket, 1);
        assert!(
            state
                .requirements
                .iter()
                .all(|row| row.requirement.identity_group.is_none())
        );
        assert!(state.to_query().is_ok());
        state.apply(&[Edit::Detach { key: 2 }]);
        assert_eq!(board(&state).items.len(), 3);
        state.requirements.retain(|row| row.requirement.blanket);
        assert!(state.to_query().is_err());
    }

    #[test]
    fn artifact_scout_matches_the_vault_row_with_its_upgrade() {
        use shpd_seedfinder_core::{
            challenges::Challenges, model::ItemSource, query::scout_matches, seed::DungeonSeed,
        };
        let world = shpd_seedfinder_session::production_scout_world(
            DungeonSeed::from_code("AAA-AAA-AAA").unwrap(),
            Challenges::NONE,
        )
        .unwrap();
        let state = AppState {
            requirements: vec![row(
                1,
                Requirement {
                    item: Some(ItemId::SandalsOfNature),
                    upgrade: UpgradeRequirement::Exact(5),
                    source: Some(ItemSource::ImpReward),
                    max_depth: Some(19),
                    ..Requirement::any(ItemKind::Artifact)
                },
            )],
            ..AppState::default()
        };
        let query = state.to_query().unwrap();
        let marks = scout_matches(&world, &query);
        assert_eq!(marks.matched_requirements, 1);
        assert_eq!(marks.matched.len(), world.items.len());
        let selected: Vec<_> = world
            .items
            .iter()
            .zip(&marks.matched)
            .filter_map(|(item, &matched)| matched.then_some(item))
            .collect();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].item, ItemId::SandalsOfNature);
        assert_eq!(selected[0].depth, 19);
        assert_eq!(selected[0].upgrade, 5);
        assert_eq!(source_label(selected[0].source), "Imp reward");
    }

    #[test]
    fn artifacts_keep_limits_and_or_groups_through_share_links() {
        use shpd_seedfinder_core::{deep_link, model::ItemSource};

        let mut state = AppState {
            requirements: [ItemId::SandalsOfNature, ItemId::HornOfPlenty]
                .into_iter()
                .zip(1..)
                .map(|(id, key)| {
                    row(
                        key,
                        Requirement {
                            item: Some(id),
                            upgrade: UpgradeRequirement::Exact(5),
                            source: Some(ItemSource::ImpReward),
                            max_depth: Some(19),
                            require_uncursed: true,
                            alternative_group: Some(1),
                            ..Requirement::any(ItemKind::Artifact)
                        },
                    )
                })
                .collect(),
            ..AppState::default()
        };
        assert!(super::ALL_KIND_CHOICES.contains(&(ItemKind::Artifact, None)));
        let chip = &entry(&state, 1).chips[0];
        assert_eq!(chip.title, "Sandals of Nature");
        assert!(chip.details.contains(&"exactly +5".to_owned()));
        assert!(chip.details.contains(&"floors 1–19".to_owned()));
        let query = state.to_query().unwrap();
        let decoded = deep_link::decode_text(&deep_link::encode_link(&query).unwrap()).unwrap();
        assert_eq!(AppState::from_query(&decoded).to_query().unwrap(), query);
        assert_eq!(query.slot_count(), 1);
        // Artifacts are unique finds: neither the cluster nor one alone stacks.
        assert!(!entry(&state, 1).stack.can_grow);
        state.apply(&[Edit::SetCount { key: 1, count: 2 }]);
        assert_eq!(state.requirements.len(), 2);
        state.apply(&[Edit::Detach { key: 1 }]);
        assert!(!entry(&state, 1).stack.can_grow);
        state.apply(&[Edit::SetCount { key: 1, count: 2 }]);
        assert_eq!(state.requirements.len(), 2);
        state.requirements[0].requirement.item = None;
        let chip = &entry(&state, state.requirements[0].key).chips[0];
        assert_eq!(
            (chip.name.as_str(), chip.title.as_str()),
            ("Artifact", "Artifact")
        );
        assert!(state.to_query().is_err());
    }

    #[test]
    fn trinkets_round_trip_in_or_groups_without_equipment_details() {
        let state = AppState {
            requirements: [ItemId::MimicTooth, ItemId::RatSkull]
                .into_iter()
                .zip(1..)
                .map(|(id, key)| {
                    row(
                        key,
                        Requirement {
                            item: Some(id),
                            alternative_group: Some(1),
                            ..Requirement::any(ItemKind::Trinket)
                        },
                    )
                })
                .collect(),
            ..AppState::default()
        };
        let chip = &entry(&state, 1).chips[0];
        assert_eq!(chip.title, "Mimic Tooth");
        assert!(chip.details.is_empty());
        let query = state.to_query().unwrap();
        assert_eq!(AppState::from_query(&query).to_query().unwrap(), query);
        assert_eq!(
            query.requirements[0].alternative_group,
            query.requirements[1].alternative_group
        );
        assert!(super::ALL_KIND_CHOICES.contains(&(ItemKind::Trinket, None)));
    }

    #[test]
    fn selected_trinket_survives_query_document_round_trips() {
        let mut state = AppState::default();
        state.requirements.push(row(
            1,
            Requirement {
                item: Some(ItemId::MimicTooth),
                select_trinket: true,
                ..Requirement::any(ItemKind::Trinket)
            },
        ));
        let query = state.to_query().unwrap();
        assert!(query.requirements[0].select_trinket);
        assert_eq!(entry(&state, 1).chips[0].details, ["choose at +3"]);
        assert_eq!(AppState::from_query(&query).to_query().unwrap(), query);
        let document = shpd_seedfinder_core::json_query::encode(&query);
        let decoded = shpd_seedfinder_core::json_query::decode(&document.to_string()).unwrap();
        assert_eq!(AppState::from_query(&decoded).to_query().unwrap(), query);
    }

    #[test]
    fn excluded_reforge_stack_and_mage_credit_survive_editor_and_share_round_trips() {
        use shpd_seedfinder_core::{
            deep_link, json_query, probability::estimate_match_probability,
        };
        let query = json_query::decode(r#"{"arcane_resin":"auto","arcane_resin_filter":{"include_mage_wand":true},"requirements":[{"item":"wand_frost","exclude_resin":true},{"item":"wand_frost"},{"item":"wand_frost"}],"floor_requirements":[{"depth":7,"feeling":"dark"}]}"#).unwrap();
        let state = AppState::load(&query);
        assert_eq!(state.to_query().unwrap(), query);
        assert!(state.requirements[0].requirement.exclude_resin);
        assert!(state.arcane_resin_filter.include_mage_wand);
        let view = board(&state);
        assert_eq!(view.items.len(), 1);
        assert_eq!(view.items[0].stack.count, 3);
        assert_eq!(
            deep_link::decode(&deep_link::encode(&query).unwrap()).unwrap(),
            query
        );
        let mut baseline = query.clone();
        baseline.arcane_resin_auto = false;
        let probability = estimate_match_probability(&query);
        assert!(probability > 0.0);
        assert!((probability - estimate_match_probability(&baseline)).abs() < 1e-12);
        assert_eq!(
            json_query::decode(&json_query::encode(&query).to_string()).unwrap(),
            query
        );
    }

    #[test]
    fn auto_resin_and_blankets_survive_editor_and_share_round_trips() {
        let query = shpd_seedfinder_core::json_query::decode(r#"{"arcane_resin":"auto","requirements":[{"item":"wand_lightning","upgrade":2},{"kind":"wand","upgrade":2,"blanket":true}]}"#).unwrap();
        let state = AppState::load(&query);
        assert!(state.needs_resin());
        let resin = board(&state).resin.expect("the query asks for resin");
        assert_eq!(resin.tags[0].text, "Auto");
        assert_eq!(state.to_query().unwrap(), query);
        let link = shpd_seedfinder_core::deep_link::encode(&state.to_query().unwrap()).unwrap();
        let restored = shpd_seedfinder_core::deep_link::decode(&link).unwrap();
        assert_eq!(AppState::from_query(&restored).to_query().unwrap(), query);
        let mut removed = state;
        removed.set_resin(None);
        assert!(!removed.needs_resin());
        assert!(removed.resin().is_none());
        assert!(!removed.to_query().unwrap().needs_resin());
        // The editor's resin condition sets the query's back as it was.
        let mut restored = removed.clone();
        restored.set_resin(AppState::load(&query).resin());
        assert_eq!(restored.to_query().unwrap(), query);
    }

    #[test]
    fn auto_trinket_defaults_on_and_preserves_saved_scope() {
        assert!(AppState::default().auto_apply_trinket);
        let legacy =
            shpd_seedfinder_core::wire::decode_query(br#"{"requirements":[{"item":"whip"}]}"#)
                .unwrap();
        assert!(!AppState::from_query(&legacy).auto_apply_trinket);
        let enabled = shpd_seedfinder_core::query::SearchQuery {
            floor_requirements: Vec::new(),
            auto_apply_trinket: true,
            ..legacy
        };
        assert!(
            AppState::from_query(&enabled)
                .to_query()
                .unwrap()
                .auto_apply_trinket
        );
    }

    #[test]
    fn quest_labels_name_every_variant() {
        assert_eq!(ghost_quest_label(GhostQuestType::FetidRat), "Fetid Rat");
        assert_eq!(
            ghost_quest_label(GhostQuestType::GnollTrickster),
            "Gnoll Trickster"
        );
        assert_eq!(ghost_quest_label(GhostQuestType::GreatCrab), "Great Crab");
        assert_eq!(
            wandmaker_quest_label(WandmakerQuestType::CorpseDust),
            "Corpse Dust"
        );
        assert_eq!(
            wandmaker_quest_label(WandmakerQuestType::ElementalEmbers),
            "Elemental Embers"
        );
        assert_eq!(
            wandmaker_quest_label(WandmakerQuestType::Rotberry),
            "Rotberry"
        );
        assert_eq!(
            blacksmith_quest_label(BlacksmithQuestType::Crystal),
            "Crystal Spire"
        );
        assert_eq!(
            blacksmith_quest_label(BlacksmithQuestType::Gnoll),
            "Gnoll Geomancer"
        );
        assert_eq!(imp_target_label(ImpQuestType::Vault), "Vault");
    }

    #[test]
    fn source_labels_name_every_item_source() {
        use shpd_seedfinder_core::model::ItemSource;

        // The source picker offers `ItemSource::ALL` verbatim, so every entry
        // needs a label; a new engine source shows up here first.
        for source in ItemSource::ALL {
            assert!(!source_label(*source).is_empty());
        }
        assert_eq!(source_label(ItemSource::ImpReward), "Imp reward");
        assert_eq!(source_label(ItemSource::VaultTreasure), "Vault treasure");
    }

    #[test]
    fn quest_rows_keep_dungeon_order_and_skip_missing_quests() {
        assert!(quest_rows(QuestSummary::default()).is_empty());

        // Seed AAA-AAA-AAA's canonical schedule.
        let summary = QuestSummary {
            ghost: Some(ScheduledQuest {
                variant: GhostQuestType::GreatCrab,
                depth: 4,
            }),
            wandmaker: Some(ScheduledQuest {
                variant: WandmakerQuestType::ElementalEmbers,
                depth: 9,
            }),
            blacksmith: Some(ScheduledQuest {
                variant: BlacksmithQuestType::Crystal,
                depth: 13,
            }),
            imp: Some(ScheduledQuest {
                variant: ImpQuestType::Vault,
                depth: 19,
            }),
        };
        assert_eq!(
            quest_rows(summary),
            vec![
                QuestRow {
                    giver: "Sad Ghost",
                    variant: "Great Crab",
                    depth: 4,
                },
                QuestRow {
                    giver: "Wandmaker",
                    variant: "Elemental Embers",
                    depth: 9,
                },
                QuestRow {
                    giver: "Blacksmith",
                    variant: "Crystal Spire",
                    depth: 13,
                },
                QuestRow {
                    giver: "Imp",
                    variant: "Vault",
                    depth: 19,
                },
            ]
        );

        let partial = QuestSummary {
            wandmaker: Some(ScheduledQuest {
                variant: WandmakerQuestType::Rotberry,
                depth: 8,
            }),
            ..QuestSummary::default()
        };
        assert_eq!(
            quest_rows(partial),
            vec![QuestRow {
                giver: "Wandmaker",
                variant: "Rotberry",
                depth: 8,
            }]
        );
    }

    #[test]
    fn wandmaker_quest_survives_the_query_round_trip() {
        let mut state = AppState::default();
        let key = state.claim_key();
        state
            .requirements
            .push(row(key, Requirement::any(ItemKind::Weapon)));
        assert_eq!(state.to_query().unwrap().wandmaker_quest, None);

        state.wandmaker_quest = Some(WandmakerQuestType::Rotberry);
        let query = state.to_query().unwrap();
        assert_eq!(query.wandmaker_quest, Some(WandmakerQuestType::Rotberry));
        assert_eq!(
            AppState::from_query(&query).wandmaker_quest,
            Some(WandmakerQuestType::Rotberry)
        );
    }

    #[test]
    fn share_links_round_trip_the_whole_editor_state() {
        use shpd_seedfinder_core::catalog::WeaponCategory;
        use shpd_seedfinder_core::challenges::Challenges;
        use shpd_seedfinder_core::deep_link;

        let mut state = AppState::default();
        let key = state.claim_key();
        state.requirements.push(row(
            key,
            Requirement {
                weapon_category: Some(WeaponCategory::Melee),
                tier: TierRequirement::AtLeast(4),
                upgrade: UpgradeRequirement::Exact(2),
                require_uncursed: true,
                max_depth: Some(9),
                ..Requirement::any(ItemKind::Weapon)
            },
        ));
        let key = state.claim_key();
        state.requirements.push(row(
            key,
            Requirement {
                item: Some(ItemId::RingTenacity),
                identity_group: Some(2),
                ..Requirement::any(ItemKind::Ring)
            },
        ));
        state.max_depth = 13;
        state.require_blacksmith = true;
        state.wandmaker_quest = Some(WandmakerQuestType::ElementalEmbers);
        state.challenges = Challenges::NO_SCROLLS;

        let query = state.to_query().unwrap();
        let link = deep_link::encode_link(&query).unwrap();
        assert!(link.starts_with(deep_link::WEB_LINK_PREFIX));
        let decoded = deep_link::decode_text(&link).unwrap();
        assert_eq!(decoded, query);

        // A received link restores editor state that produces the identical
        // query, so copying the link again shares the same search.
        let restored = AppState::from_query(&decoded);
        assert_eq!(restored.to_query().unwrap(), query);
        // Loading it into the editor drops the stack label one ring cannot
        // use, and changes nothing the search reads.
        let loaded = AppState::load(&decoded);
        assert_eq!(loaded.requirements[1].requirement.identity_group, None);
        assert_eq!(loaded.to_query().unwrap().slot_count(), query.slot_count());

        // The custom-scheme form the desktop handler receives decodes too.
        let code = link.strip_prefix(deep_link::WEB_LINK_PREFIX).unwrap();
        let uri = format!("{}://q/{code}", deep_link::URI_SCHEME);
        assert_eq!(deep_link::decode_text(&uri).unwrap(), query);
    }

    #[test]
    fn share_links_carry_the_v4_effects_and_the_weapon_ceiling() {
        use shpd_seedfinder_core::catalog::{Effect, WeaponEffect};
        use shpd_seedfinder_core::deep_link;
        use shpd_seedfinder_core::query::{EffectRequirement, EffectSet};

        // A set naming Crystal only fits the wider effect mask of link format
        // three, and +5 is the ceiling weapons alone reach.
        let mut state = AppState::default();
        let key = state.claim_key();
        state.requirements.push(row(
            key,
            Requirement {
                upgrade: UpgradeRequirement::Exact(5),
                effect: EffectRequirement::OneOf(
                    EffectSet::from_effects([
                        Effect::Weapon(WeaponEffect::Blazing),
                        Effect::Weapon(WeaponEffect::Crystal),
                    ])
                    .unwrap(),
                ),
                ..Requirement::any(ItemKind::Weapon)
            },
        ));

        let query = state.to_query().unwrap();
        let link = deep_link::encode_link(&query).unwrap();
        let decoded = deep_link::decode_text(&link).unwrap();
        assert_eq!(decoded, query);

        let restored = AppState::load(&decoded);
        assert_eq!(restored.to_query().unwrap(), query);
        assert_eq!(
            entry(&restored, 1).chips[0].details,
            ["exactly +5", "effect: Blazing/Crystal"]
        );
    }

    #[test]
    fn query_drops_blacksmith_requirement_at_depth_fourteen() {
        let mut state = AppState::default();
        let key = state.claim_key();
        state
            .requirements
            .push(row(key, Requirement::any(ItemKind::Weapon)));
        state.require_blacksmith = true;
        state.max_depth = 14;
        assert!(!state.to_query().unwrap().require_blacksmith);
        state.max_depth = 13;
        assert!(state.to_query().unwrap().require_blacksmith);
    }

    #[test]
    fn the_board_collapses_alternatives_and_stacks_into_one_entry_each() {
        let mut state = AppState::default();
        let spear = state.claim_key();
        state.requirements.push(row(
            spear,
            Requirement {
                item: Some(ItemId::Spear),
                upgrade: UpgradeRequirement::Exact(3),
                ..Requirement::any(ItemKind::Weapon)
            },
        ));
        let ring = state.claim_key();
        state
            .requirements
            .push(row(ring, Requirement::any(ItemKind::Ring)));
        assert_eq!(board(&state).items.len(), 2);

        // Dropping the ring on the spear makes one either/or entry, and one
        // slot for the engine; the editor follows the ring.
        let joined = state.apply(&[Edit::Join {
            source: ring,
            target: spear,
        }]);
        assert!(joined.changed);
        assert_eq!(joined.focus, Some(ring));
        assert_eq!(board(&state).items.len(), 1);
        let item = entry(&state, spear);
        assert_eq!(item.members, [spear, ring]);
        assert_eq!(item.label.as_deref(), Some("Any of 2"));
        assert_eq!(state.unvalidated_query().slot_count(), 1);

        // A cluster spanning two categories cannot anchor a stack: a copy
        // would have to name a kind, and "spear or ring" names none.
        assert!(
            !state
                .apply(&[Edit::SetCount {
                    key: spear,
                    count: 2
                }])
                .changed
        );
        assert_eq!(state.requirements.len(), 2);
        assert!(!entry(&state, spear).stack.can_grow);

        // Pulling the ring back out leaves a plain spear chip, which can.
        state.apply(&[Edit::Detach { key: ring }]);
        assert_eq!(board(&state).items.len(), 2);
        assert!(entry(&state, spear).stack.can_grow);
        let grown = state.apply(&[Edit::SetCount {
            key: spear,
            count: 2,
        }]);
        assert_eq!(state.requirements.len(), 3);
        // The copy took the next key in line.
        assert_eq!(state.requirements[1].key, 3);
        assert_eq!(grown.next_key, 4);
        assert_eq!(state.claim_key(), 4);
        assert_eq!(entry(&state, spear).stack.count, 2);
        assert_eq!(entry(&state, ring).stack.count, 1);
        assert!(
            state
                .requirements
                .iter()
                .all(|row| row.requirement.identity_group.is_none())
        );
        assert!(state.to_query().is_ok());

        // Removing the spear takes its hidden copy with it.
        state.apply(&[Edit::Remove { key: spear }]);
        assert_eq!(state.requirements.len(), 1);
        assert_eq!(state.requirements[0].key, ring);
    }

    #[test]
    fn an_edit_that_changes_nothing_leaves_the_rows_alone() {
        let mut state = AppState::default();
        let key = state.claim_key();
        state
            .requirements
            .push(row(key, Requirement::any(ItemKind::Wand)));
        let before = state.requirements.clone();
        // A lone chip has no cluster to leave, and an unknown key is nothing.
        for edit in [Edit::Detach { key }, Edit::Remove { key: 99 }] {
            let result = state.apply(&[edit]);
            assert!(!result.changed);
            assert!(result.refused.is_none());
            assert_eq!(state.requirements, before);
        }
    }

    #[test]
    fn a_combined_level_stack_is_built_and_checked_through_the_editor() {
        let mut state = AppState::default();
        let ring = Requirement {
            item: Some(ItemId::RingMight),
            ..Requirement::any(ItemKind::Ring)
        };
        let draft = sheet(&mut state, None, ring, (2, Some(4), None));
        let saved = state.save(&draft).unwrap();
        let key = draft.key.unwrap();
        assert_eq!(saved.focus, Some(key));
        assert_eq!(state.requirements.len(), 2);
        assert!(
            state.requirements.iter().all(|row| row
                .requirement
                .level_sum
                .map(|sum| sum.minimum_total)
                == Some(4))
        );
        let item = entry(&state, key);
        assert_eq!((item.stack.count, item.stack.total), (2, Some(4)));
        assert!(!item.chips[0].in_cluster);
        assert!(state.to_query().is_ok());

        // A ring reaches +4 (five levels), but only one per world — the Imp
        // vault's prize; every other ring stops at +2 (three levels). Two
        // rings therefore reach eight levels together, three eleven.
        assert_eq!(item.stack.level_capacity, 8);
        state.apply(&[Edit::SetCount { key, count: 3 }]);
        assert_eq!(entry(&state, key).stack.level_capacity, 11);
        state.apply(&[Edit::SetCount { key, count: 2 }]);

        // The badge lowers the total without going through the sheet, and
        // never past what the stack can reach.
        state.apply(&[Edit::SetTotal {
            key,
            total: Some(3),
        }]);
        assert_eq!(entry(&state, key).stack.total, Some(3));
        state.apply(&[Edit::SetTotal {
            key,
            total: Some(9),
        }]);
        assert_eq!(entry(&state, key).stack.total, Some(8));
        assert_eq!(
            entry(&state, key).badges.total.unwrap().text,
            "\u{3a3} \u{2265} 8"
        );

        // Giving up on counting levels returns the stack to plain repeats.
        state.apply(&[Edit::ToggleLevels { key }]);
        assert!(
            state
                .requirements
                .iter()
                .all(|row| row.requirement.level_sum.is_none())
        );
        assert_eq!(entry(&state, key).stack.count, 2);
        assert!(state.to_query().is_ok());
        // …and the menu turns it back on at one level per item.
        state.apply(&[Edit::ToggleLevels { key }]);
        assert_eq!(entry(&state, key).stack.total, Some(2));
    }

    #[test]
    fn a_stack_of_copies_carries_its_own_floor_limit() {
        let mut state = AppState::default();
        let armor = Requirement {
            upgrade: UpgradeRequirement::Exact(3),
            max_depth: Some(4),
            ..Requirement::any(ItemKind::Armor)
        };
        let draft = sheet(&mut state, None, armor, (2, None, Some(9)));
        state.save(&draft).unwrap();
        let item = entry(&state, draft.key.unwrap());
        assert_eq!(item.stack.count, 2);
        assert_eq!(item.stack.copy_depth, Some(9));
        // The named +3 armor keeps its own floor; the copy keeps the other.
        assert_eq!(state.requirements[0].requirement.max_depth, Some(4));
        assert_eq!(state.requirements[1].requirement.max_depth, Some(9));
        assert!(state.to_query().is_ok());

        // Saving the chip as it stands gives the rows back untouched.
        let before = state.requirements.clone();
        let again = state.open_sheet(Some(item.members[0]), false);
        let unchanged = state.save(&again).unwrap();
        assert!(!unchanged.changed);
        assert_eq!(unchanged.focus, Some(item.members[0]));
        assert_eq!(state.requirements, before);
    }

    #[test]
    fn a_new_sheet_saves_under_its_key_and_a_blanket_follows_the_first_row() {
        let mut state = AppState::default();
        let wand = Requirement {
            item: Some(ItemId::WandFrost),
            ..Requirement::any(ItemKind::Wand)
        };
        let draft = sheet(&mut state, None, wand, (1, None, None));
        state.save(&draft).unwrap();
        assert_eq!(state.requirements[0].key, draft.key.unwrap());

        // A new blanket starts from the kind the ordinary rows ask for, and
        // cancelling a sheet costs only a key.
        let blanket = state.open_sheet(None, true);
        assert!(blanket.blanket);
        assert_eq!(blanket.requirement.kind, ItemKind::Wand);
        let result = state.save(&blanket).unwrap();
        assert_eq!(state.requirements.len(), 2);
        assert!(state.requirements[1].requirement.blanket);
        assert_eq!(state.requirements[1].key, blanket.key.unwrap());
        assert!(result.next_key > blanket.key.unwrap());
    }

    #[test]
    fn the_editor_refuses_a_sheet_that_would_break_the_list() {
        let mut state = AppState::default();
        let tooth = Requirement {
            item: Some(ItemId::MimicTooth),
            ..Requirement::any(ItemKind::Trinket)
        };
        let first = sheet(&mut state, None, tooth, (1, None, None));
        state.save(&first).unwrap();
        let duplicate = sheet(&mut state, None, tooth, (1, None, None));
        assert_eq!(
            state.save(&duplicate).unwrap_err(),
            [editor::DUPLICATE_TRINKET]
        );
        assert_eq!(state.requirements.len(), 1);

        // A stacked cluster of wands cannot take a ring: its copies would
        // have to name a kind.
        let mut state = AppState::default();
        for _ in 0..2 {
            let key = state.claim_key();
            state.requirements.push(row(
                key,
                Requirement {
                    alternative_group: Some(1),
                    ..Requirement::any(ItemKind::Wand)
                },
            ));
        }
        state.apply(&[Edit::SetCount { key: 1, count: 2 }]);
        assert_eq!(entry(&state, 1).stack.count, 2);
        let before = state.requirements.clone();
        let ring = Requirement {
            alternative_group: Some(1),
            ..Requirement::any(ItemKind::Ring)
        };
        let draft = sheet(&mut state, Some(2), ring, (1, None, None));
        assert!(draft.in_cluster);
        assert_eq!(
            state.save(&draft).unwrap_err(),
            ["Copies can only be grouped with the same item type."]
        );
        assert_eq!(state.requirements, before);
    }
}

#[cfg(test)]
mod floor_requirement_tests {
    use super::*;
    use shpd_seedfinder_core::{deep_link, json_query};

    #[test]
    fn farming_floors_are_independent_and_round_trip_editor_and_links() {
        let mut state = AppState {
            max_depth: 4,
            ..AppState::default()
        };
        for depth in [22, 7, 17] {
            state.toggle_farming_floor(depth);
        }
        assert_eq!(state.max_depth, 22);
        assert_eq!(
            state
                .floor_requirements
                .iter()
                .map(|floor| floor.depth)
                .collect::<Vec<_>>(),
            FARMING_FLOORS
        );
        assert!(state.floor_requirements.iter().all(is_farming_requirement));
        let query = state.to_query().unwrap();
        for restored in [
            json_query::decode(&json_query::encode(&query).to_string()).unwrap(),
            deep_link::decode(&deep_link::encode(&query).unwrap()).unwrap(),
        ] {
            assert_eq!(AppState::from_query(&restored).unvalidated_query(), query);
        }
        state.toggle_farming_floor(17);
        assert_eq!(
            state
                .floor_requirements
                .iter()
                .map(|floor| floor.depth)
                .collect::<Vec<_>>(),
            [7, 22]
        );
        assert_eq!(state.max_depth, 22);
        state.max_depth = 16;
        assert!(state.to_query().is_err());
    }

    #[test]
    fn general_filters_survive_and_toggling_replaces_only_the_selected_floor() {
        let query = json_query::decode(r#"{"requirements":[],"floor_requirements":[{"depth":7,"feeling":"water","rooms":["garden"]},{"depth":9,"feeling":"secrets","rooms":["secret_library"]}]}"#).unwrap();
        let mut state = AppState::from_query(&query);
        assert_eq!(state.unvalidated_query(), query);
        state.toggle_farming_floor(7);
        assert!(is_farming_requirement(&state.floor_requirements[0]));
        assert_eq!(state.floor_requirements[1], query.floor_requirements[1]);
        assert!(state.to_query().is_ok());
    }
}
