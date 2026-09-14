//! Embedded, revision-pinned game textures, shared by every engine bridge.
//! IDs are an allowlist, never filesystem paths. See the asset attribution.

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
pub struct MapAsset {
    pub id: &'static str,
    pub width: u16,
    pub height: u16,
    pub sha256: &'static str,
    #[cfg_attr(feature = "json-query", serde(skip))]
    pub png: &'static [u8],
}

pub const SOURCE_REVISION: &str = "2bb34a4e91d29c8785a9363cad6ddfe5122b1d4f";

pub const ASSETS: &[MapAsset] = &[
    MapAsset {
        id: "tiles_sewers.png",
        width: 256,
        height: 256,
        sha256: "4a097755d0b0335b04238ef5e848da45d9c313348865e4dc17ccac44dc487567",
        png: include_bytes!("../../assets/level-map/tiles_sewers.png"),
    },
    MapAsset {
        id: "tiles_prison.png",
        width: 256,
        height: 256,
        sha256: "67e88eb7596b2d963b458480f6b025aa9107ef610f291961d0e257d8b68fd5cb",
        png: include_bytes!("../../assets/level-map/tiles_prison.png"),
    },
    MapAsset {
        id: "tiles_caves.png",
        width: 256,
        height: 256,
        sha256: "85141c7c7ad9450bb324a79707b4645343542b74160a98ff5fdf0c6f82968056",
        png: include_bytes!("../../assets/level-map/tiles_caves.png"),
    },
    MapAsset {
        id: "tiles_city.png",
        width: 256,
        height: 256,
        sha256: "a1f5d8b76321e2774811e7a4144832ab50666c5cebca52334824c087d3d97b80",
        png: include_bytes!("../../assets/level-map/tiles_city.png"),
    },
    MapAsset {
        id: "tiles_halls.png",
        width: 256,
        height: 256,
        sha256: "6c8a81bd9e32832e78f647fc833c76ab8db640e56de03255efdf5644075316f0",
        png: include_bytes!("../../assets/level-map/tiles_halls.png"),
    },
    MapAsset {
        id: "water0.png",
        width: 32,
        height: 32,
        sha256: "4d50a06e9381824d50495c6df332c4644349021693cdbceef1a80f25ffae9202",
        png: include_bytes!("../../assets/level-map/water0.png"),
    },
    MapAsset {
        id: "water1.png",
        width: 32,
        height: 32,
        sha256: "16b7cd5e206e0938694112f27ab8bf5ec47cb7c3080b90ffc697cd929872f71f",
        png: include_bytes!("../../assets/level-map/water1.png"),
    },
    MapAsset {
        id: "water2.png",
        width: 32,
        height: 32,
        sha256: "e3cb69225c9d27234474d80dbe30fbb411d0042fea8e3ebaeba519a1f5f0b894",
        png: include_bytes!("../../assets/level-map/water2.png"),
    },
    MapAsset {
        id: "water3.png",
        width: 32,
        height: 32,
        sha256: "35811bc2cfccd53a7278748e4202ed6a175d6993158f1cd48679e4392ae4ca69",
        png: include_bytes!("../../assets/level-map/water3.png"),
    },
    MapAsset {
        id: "water4.png",
        width: 32,
        height: 32,
        sha256: "ada5c5a29bb564b15380fa07c161004b3f11bc6c5907bbda34cf539d19f5a84c",
        png: include_bytes!("../../assets/level-map/water4.png"),
    },
    MapAsset {
        id: "terrain_features.png",
        width: 256,
        height: 256,
        sha256: "1786acbca33b65fbb6f8e671b811201263f56cbb26d9a7741e0a698421460bec",
        png: include_bytes!("../../assets/level-map/terrain_features.png"),
    },
    MapAsset {
        id: "tiles_caves_crystal.png",
        width: 256,
        height: 256,
        sha256: "e9d55a8059fef86c2fd43b3124455e62335cfb6e5473b6b22423a0871ccbed9c",
        png: include_bytes!("../../assets/level-map/tiles_caves_crystal.png"),
    },
    MapAsset {
        id: "tiles_caves_gnoll.png",
        width: 256,
        height: 256,
        sha256: "8258601b286b685530e26835ff20962ad4d6b43a64bc3458653e618088701ef5",
        png: include_bytes!("../../assets/level-map/tiles_caves_gnoll.png"),
    },
    MapAsset {
        id: "caves_quest.png",
        width: 64,
        height: 128,
        sha256: "7b5917a106271f320f53ffc84df1a40447b2c283e31309bacc803cac3eadfcd7",
        png: include_bytes!("../../assets/level-map/caves_quest.png"),
    },
    MapAsset {
        id: "city_quest.png",
        width: 256,
        height: 256,
        sha256: "8782ef5007a841b2f13abc18b31983335ca193daed590760439c44410781b529",
        png: include_bytes!("../../assets/level-map/city_quest.png"),
    },
    MapAsset {
        id: "occlusion_shadows.png",
        width: 128,
        height: 128,
        sha256: "f51350203bf898a544c03fd87f36607c225619ee5e785ecdd7940ac7fe9213f2",
        png: include_bytes!("../../assets/level-map/occlusion_shadows.png"),
    },
    MapAsset {
        id: "raised_terrain.png",
        width: 64,
        height: 128,
        sha256: "405c2c34b3f8815a78ad78a68ed89cc8fd5b8cbdcec88f31532ec266a394b76b",
        png: include_bytes!("../../assets/level-map/raised_terrain.png"),
    },
];

#[must_use]
pub fn get(id: &str) -> Option<&'static MapAsset> {
    ASSETS.iter().find(|asset| asset.id == id)
}
