//! Daily fixtures come from the unmodified official v4.0.0 JAR:
//! `tooling/oracle-4.0/run.sh --daily 2026-09-25 --floors 1 --format json`.

use shpd_seedfinder_core::{
    challenges::Challenges,
    item_mappings::item_mappings,
    main_world::generate_main_world_with_trinket,
    rng::seed_for_depth,
    seed::DungeonSeed,
    trinkets::trinket_order,
    wire::{decode_scout_seed, decode_scout_world, encode_scout_world_with_artifacts},
};

#[test]
fn daily_identity_and_initialization_match_the_official_jar() {
    let seed = DungeonSeed::from_daily_date("2026-09-25").unwrap();
    assert_eq!(seed.value(), 7_219_798_078_976);
    assert_eq!(
        seed_for_depth(i64::try_from(seed.value()).unwrap(), 1, 0),
        3_645_134_637_768_325_796
    );
    let mappings = item_mappings(seed);
    assert_eq!(mappings.scrolls[0].appearance, "ODAL");
    assert_eq!(
        mappings
            .rings
            .map(|mapping| mapping.appearance.to_ascii_lowercase()),
        [
            "onyx",
            "garnet",
            "tourmaline",
            "diamond",
            "emerald",
            "quartz",
            "amethyst",
            "sapphire",
            "opal",
            "topaz",
            "ruby",
            "agate"
        ]
    );
    // Scout packets retain the date, including with an explicit trinket.
    let selected = trinket_order(seed)[0];
    let world =
        generate_main_world_with_trinket(seed, 1, Challenges::NONE, Some(selected)).unwrap();
    let packet = encode_scout_world_with_artifacts(&world, Some(selected)).unwrap();
    assert_eq!(decode_scout_world(&packet).unwrap(), world);
    assert_eq!(decode_scout_seed(b"2026-09-25").unwrap(), seed);
    assert!(!world.items.is_empty());
}
