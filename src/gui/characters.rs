//! What the GUI knows about the characters (bodies and their animation), published by
//! `engine::animation` and read by the character page and the Dev page. Moved out of `GuiState`
//! (2026-11-09, character animation rung 2a) as one cluster, so the GUI state's monolith does not
//! grow with each rung (tests/file_size_ratchet.rs).

/// `GuiState::characters`.
#[derive(Debug, Clone)]
pub struct CharactersGui {
    /// The bodies a character can be drawn with (data/characters/characters.ron's choice ids,
    /// "female" and "male"), published by engine::animation when the character data loads; the
    /// character page's body chooser lists them. Empty until then.
    pub body_choices: Vec<String>,
    /// Each body's shape sliders, by its choice id (2026-11-09; the body files' `shape.sliders`,
    /// published with `body_choices`): the character page draws the chosen body's (build,
    /// proportions, ancestry) and writes their values into `appearance.shape`.
    pub body_sliders: std::collections::BTreeMap<String, Vec<humanity_core::player_look::SliderBlock>>,
    /// How tall the player's own body stands, metres (character animation rung 2a, 2026-11-09):
    /// measured from the chosen body's mesh with the look's height, build and ancestry
    /// (`LoadedBody::standing_height_m` through `shape_for`), published by engine::animation when
    /// the look changes. The character page shows it beside the height slider ("1.62 m"). None
    /// until the character data loads.
    pub my_height_m: Option<f32>,
    /// The Dev page's Show skeletons (2026-11-09; showcase `skeletons`): every drawn body's joints
    /// and bones on top of everything (engine::animation), so a reshaped skeleton can be seen to
    /// follow its body; with the feet on, where each foot found the ground too. Off by default;
    /// never saved.
    pub show_skeletons: bool,
    /// The Dev page's Feet on the world (2026-11-09, character animation rung 2a; showcase `feet`):
    /// on, every body's feet go onto the ground under them (engine::animation, the animation mode
    /// saying how); off, every body is drawn as its clips make it, as rung 1 did, for a same-boot A/B.
    /// On by default; never saved.
    pub feet_on_world: bool,
    /// Each body's face and body detail sliders by its choice id, in groups (2026-11-09, the look;
    /// data/characters/details.ron, published with `body_choices`): the character page draws the
    /// chosen body's and writes their values into `appearance.detail`.
    pub detail_sliders: std::collections::BTreeMap<String, Vec<humanity_core::player_look::DetailBlock>>,
    /// The hair styles, brows and lashes a character can wear (garment ids of
    /// data/characters/garments.ron), by kind: "hair", "brows", "lashes". Empty until the look loads.
    pub wearables: std::collections::BTreeMap<String, Vec<String>>,
    /// Done was pressed on the character page with a look that may have changed: the engine sends
    /// it to the shared world (`engine::net_route`, at most once every ten seconds, the latest).
    pub look_send: bool,
    /// When the last look went out (seconds of the engine's own clock), for that wait.
    pub look_sent_at: Option<f64>,
}

impl Default for CharactersGui {
    fn default() -> Self {
        CharactersGui {
            body_choices: Vec::new(),
            body_sliders: Default::default(),
            my_height_m: None,
            show_skeletons: false,
            feet_on_world: true,
            detail_sliders: Default::default(),
            wearables: Default::default(),
            look_send: false,
            look_sent_at: None,
        }
    }
}
