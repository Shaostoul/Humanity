//! Whether the person is in a world, and the world held still and quiet until they are (BUG-253,
//! 2026-11-06).
//!
//! The operator, 2026-11-05, with a screenshot of the Who / Where picker and four garden toasts
//! over it ("The sunflower plants in oilbed 8 are flowering, and indoors nothing carries their
//! pollen..."): "I hadn't entered the world yet. I clicked to view characters. Then the game seems
//! to have launched as per the bottom middle popups."
//!
//! WHAT HAPPENED. The home's save is put on the world at startup (lib.rs, so the pages can show
//! the character's things), Play loads the 3D world behind the picker (its stage), and the picker
//! loads the character it preselects onto that world (engine/save_slot.rs `apply_pending_load`)
//! with the offline catch-up. The world's systems ticked every frame from startup, and what they
//! said went straight to toasts: the farm's first step told the flowering beds that nothing
//! pollinates them, over the picker, before the person had chosen to enter anything.
//!
//! THE RULE: nothing of a world runs or speaks before the person enters it.
//! - The world RUNS once the person has stood in it this session (`world_entered`), and not while
//!   the character picker is open, where they choose which world to enter (`note_world_frame`).
//!   Held, its systems still take what a page asks of them (a transfer on the Inventory page) but
//!   no time passes for them: nothing grows, ages, flies, wears or is eaten (engine/world_hold.rs).
//! - What the world SAYS while held waits (`world_notice`, `held_world_notices`) and is shown on
//!   the frame the person enters. A save loaded onto the world while it is held drops what the
//!   world before it said (`drop_held_world_notices`: that world is gone), and the farm tells the
//!   loaded world's beds again (farming's resume).
//! - No time is lost: a save written while the world is held is stamped with the moment the world
//!   is current to, so the next load's catch-up counts the held time as time away (save_load.rs
//!   `WorldHold`).

use crate::gui::{GuiPage, GuiState};

impl GuiState {
    /// Whether the character picker is open: Play's Who / Where (showroom mode 0), or its Edit
    /// look (whose Done goes back to the picker). The wardrobe and the mirror in the home are not.
    pub fn picker_open(&self) -> bool {
        self.showroom_active && (self.showroom_mode == 0 || self.showroom_return_to_picker)
    }

    /// Whether the person stands in a world now: loaded, no page over it, the picker closed.
    pub fn in_a_world(&self, world_loaded: bool) -> bool {
        world_loaded && self.active_page == GuiPage::None && !self.picker_open()
    }

    /// Once a frame, before the world's systems tick: whether the world runs this frame. The first
    /// frame the person stands in a world marks it entered; from then on it runs, except while the
    /// picker is open (a page over the world after entering does not stop it, as it never has). On
    /// a frame it runs, what it said while it was held is shown.
    pub fn note_world_frame(&mut self, world_loaded: bool) -> bool {
        if self.in_a_world(world_loaded) {
            self.world_entered = true;
        }
        self.world_running = self.world_entered && !self.picker_open();
        if self.world_running && !self.held_world_notices.is_empty() {
            let held = std::mem::take(&mut self.held_world_notices);
            self.pending_notices.extend(held);
        }
        self.world_running
    }

    /// A notice from the world: shown now while the world runs, held for the person's entry while
    /// it does not (the same line twice is held once).
    pub fn world_notice(&mut self, msg: String) {
        if self.world_running {
            self.pending_notices.push(msg);
        } else if !self.held_world_notices.contains(&msg) {
            self.held_world_notices.push(msg);
        }
    }

    /// A save was put on the world: what the world before it said while held was about a world
    /// that is gone.
    pub fn drop_held_world_notices(&mut self) {
        self.held_world_notices.clear();
    }
}
