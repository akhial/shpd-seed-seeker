//! Small official-JAR corpus shared by the region regression tests.
use crate::{
    catalog::{ItemKind, item},
    level::Level,
    model::WorldItem,
};
use serde_json::{Value, json};
use std::sync::LazyLock;

static FIXTURE: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../tests/fixtures/main-floors-v4.0.2.json")).unwrap()
});

pub(crate) fn floor(code: &str, mask: u16, depth: u32) -> &'static Value {
    FIXTURE["floors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["seed"] == code && row["challenges"] == mask && row["depth"] == depth)
        .unwrap_or_else(|| panic!("missing official fixture: {code} mask {mask} depth {depth}"))
}

pub(crate) fn assert_floor(
    code: &str,
    mask: u16,
    level: &Level,
    items: &[WorldItem],
    mobs: impl Iterator<Item = (String, usize)>,
) {
    let row = floor(code, mask, level.depth);
    assert_eq!(
        json!([level.width(), level.height()]),
        row["size"],
        "{code} size"
    );
    assert_eq!(
        json!(level.java_map_hash()),
        row["mapHash"],
        "{code} depth {}",
        level.depth
    );
    assert_eq!(json!(level.entrance()), row["entrance"]);
    assert_eq!(json!(level.exit()), row["exit"]);
    assert_eq!(
        format!("{:?}", level.feeling).to_uppercase(),
        row["feeling"]
    );
    let occupied: Vec<_> = level
        .mob_cells
        .iter()
        .enumerate()
        .filter_map(|(cell, &occupied)| occupied.then_some(cell))
        .collect();
    assert_eq!(
        json!(occupied),
        row["occupied"],
        "{code} depth {} occupied",
        level.depth
    );
    let mut mobs: Vec<_> = mobs
        .map(|(kind, cell)| (kind.to_lowercase(), cell))
        .collect();
    mobs.sort();
    assert_eq!(
        json!(mobs),
        row["mobs"],
        "{code} depth {} mobs",
        level.depth
    );
    assert_items(row, items);
}

pub(crate) fn assert_items(row: &Value, items: &[WorldItem]) {
    let mut actual: Vec<_> = items
        .iter()
        .filter(|entry| {
            !matches!(
                item(entry.item).kind,
                ItemKind::Trinket | ItemKind::Artifact
            )
        })
        .map(|entry| {
            // The generic Java recorder assigns crystal choices by cell order.
            // Room topology and choice constraints have dedicated tests instead.
            json!([
                item(entry.item).stable_id,
                entry.upgrade,
                entry.cursed,
                entry.effect.map_or("-", crate::catalog::Effect::wire_name),
                format!("{:?}", entry.source).to_lowercase(),
                entry.secret
            ])
        })
        .collect();
    actual.sort_by_key(Value::to_string);
    let mut expected = row["items"].as_array().unwrap().clone();
    expected.sort_by_key(Value::to_string);
    assert_eq!(
        actual, expected,
        "{} depth {} equipment",
        row["seed"], row["depth"]
    );
}
