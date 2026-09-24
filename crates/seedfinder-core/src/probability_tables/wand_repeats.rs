//! WDR2: u32 sample counts for eight profiles, then u32 presence counts by
//! profile, vault eligibility, upgrade/reward condition, copies, and depth prefix.
use super::embedded::WAND_REPEATS as DATA;
use crate::{catalog::ItemId, model::ItemSource, vault_loot::EXCLUDED_WANDS};

pub const PROFILES: usize = 8;
pub const BANDS: usize = 4 + 4 * 5;
pub const ROWS: usize = 2 * BANDS * 4 * 24;
pub const HEADER: usize = 4 + PROFILES * 4;

#[must_use]
pub fn class(identity: ItemId) -> usize {
    usize::from(EXCLUDED_WANDS.contains(&identity))
}

/// The first four bands require every copy to meet a minimum upgrade.
/// Other bands require one copy from a source, at or above a minimum upgrade.
#[must_use]
pub fn anchor_band(source: Option<ItemSource>, minimum: usize) -> Option<usize> {
    if minimum > 4 {
        return None;
    }
    let group = match source {
        None => 0,
        Some(ItemSource::WandmakerReward) => 1,
        Some(ItemSource::ImpReward) => 2,
        Some(ItemSource::VaultTreasure) => 3,
        _ => return None,
    };
    Some(4 + group * 5 + minimum)
}

pub(crate) fn probability(
    profile: usize,
    identity: ItemId,
    band: usize,
    copies: usize,
    depth: usize,
) -> Option<f64> {
    if !(1..=24).contains(&depth) {
        return None;
    }
    // Brewing cannot affect the first two floors. Use their larger canonical
    // sample rather than introduce profile-specific sampling noise.
    let profile = if depth <= 2 { 0 } else { profile };
    let samples = read(4 + profile * 4);
    if samples == 0 || band >= BANDS || !(2..=4).contains(&copies) {
        return None;
    }
    let group = class(identity);
    let index = (((profile * 2 + group) * BANDS + band) * 4 + copies - 1) * 24 + depth - 1;
    let count = read(HEADER + index * 4);
    // Sparse cells retain the analytical model rather than asserting absence.
    (count >= 30)
        .then(|| f64::from(count) / f64::from(samples) / if group == 0 { 10.0 } else { 3.0 })
}

fn read(offset: usize) -> u32 {
    u32::from_le_bytes(DATA[offset..offset + 4].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baked_availability_is_bounded_and_monotone() {
        assert_eq!(&DATA[..4], b"WDR2");
        assert_eq!(DATA.len(), HEADER + PROFILES * ROWS * 4);
        assert_eq!(crate::generator::WAND_ITEMS.len(), 13);
        for profile in 0..PROFILES {
            let samples = read(4 + profile * 4);
            assert!(samples >= 65_536);
            for group in 0..2 {
                for band in 0..BANDS {
                    for copies in 0..4 {
                        let row = ((profile * 2 + group) * BANDS + band) * 4 + copies;
                        for depth in 0..24 {
                            let at = HEADER + (row * 24 + depth) * 4;
                            let count = read(at);
                            assert!(count <= samples * if group == 0 { 10 } else { 3 });
                            if depth > 0 {
                                assert!(count >= read(at - 4));
                            }
                            if copies > 0 {
                                assert!(count <= read(at - 24 * 4));
                            }
                            if (band < 4 && band > 0) || (band >= 4 && (band - 4) % 5 > 0) {
                                assert!(count <= read(at - 4 * 24 * 4));
                            }
                            if band >= 4 {
                                let broad = read(at - band * 4 * 24 * 4);
                                assert!(count <= broad);
                                if band == 4 {
                                    assert_eq!(count, broad);
                                }
                            }
                            if group == 1
                                && band >= anchor_band(Some(ItemSource::VaultTreasure), 0).unwrap()
                            {
                                assert_eq!(count, 0, "excluded wand in the vault");
                            }
                        }
                    }
                }
            }
        }
    }
}
