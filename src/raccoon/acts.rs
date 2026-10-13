//! The raccoon's moveset: every activity it can be in, with the timing,
//! priority and weighting that drive the brain.
//!
//! Think of it as a boss's move table. Each row has a duration range, a
//! cooldown, a priority used when something tries to interrupt it, and a
//! weight used when the brain picks something to do on its own.

/// Everything the raccoon can be doing.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Act {
    // --- idling around ---
    Idle,
    LookAround,
    Sit,
    Loaf,
    Sleep,
    Wake,
    Yawn,
    Stretch,
    Groom,
    WashPaws,
    Scratch,
    Sniff,
    Dig,
    Snack,
    Wave,
    Dance,
    TailChase,
    RollOver,
    Sneeze,
    Hop,
    Flip,
    Curious,
    // --- getting around ---
    Wander,
    Run,
    Explore,
    Jump,
    Climb,
    Mantle,
    FollowCursor,
    ChaseCursor,
    Stalk,
    Pounce,
    Fall,
    Land,
    // --- reactions ---
    Alert,
    Panic,
    Hot,
    Tired,
    Cheer,
    Stuffed,
    Watch,
    Petted,
    Booped,
    Annoyed,
    // --- being handled ---
    Dragged,
    Thrown,
    Bounce,
    Splat,
    Dizzy,
    GetUp,
}

/// How an activity gets chosen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Picked at random by weight when nothing else is going on.
    Ambient,
    /// Picked at random, but only when the cursor is in a suitable spot.
    Social,
    /// Only entered as part of a plan or by the physics (jumping, falling...).
    Internal,
    /// Driven by the machine or by the user poking at him.
    Reaction,
}

pub struct Def {
    pub act: Act,
    pub label: &'static str,
    pub kind: Kind,
    /// Seconds, as a random range. `(0, 0)` means "until done" (arrival,
    /// landing, release...).
    pub dur: (f32, f32),
    pub cooldown: f32,
    pub weight: f32,
    /// Higher wins when a reaction tries to cut in.
    pub priority: u8,
    /// Whether an equal-or-higher priority reaction may cut it short.
    pub interruptible: bool,
}

use Act::*;
use Kind::*;

const fn d(
    act: Act,
    label: &'static str,
    kind: Kind,
    dur: (f32, f32),
    cooldown: f32,
    weight: f32,
    priority: u8,
    interruptible: bool,
) -> Def {
    Def { act, label, kind, dur, cooldown, weight, priority, interruptible }
}

const UNTIL: (f32, f32) = (0.0, 0.0);

pub static DEFS: &[Def] = &[
    d(Idle, "Idle", Ambient, (2.0, 4.0), 0.0, 16.0, 0, true),
    d(LookAround, "Look around", Ambient, (2.6, 4.0), 3.0, 9.0, 0, true),
    d(Sit, "Sit", Ambient, (4.0, 8.0), 4.0, 8.0, 0, true),
    d(Loaf, "Loaf", Ambient, (5.0, 10.0), 10.0, 4.0, 0, true),
    d(Sleep, "Sleep", Reaction, (25.0, 60.0), 20.0, 0.0, 1, true),
    d(Wake, "Wake up", Internal, (1.2, 1.2), 2.0, 0.0, 2, false),
    d(Yawn, "Yawn", Ambient, (1.6, 1.6), 12.0, 2.0, 0, true),
    d(Stretch, "Stretch", Ambient, (1.8, 1.8), 12.0, 4.0, 0, true),
    d(Groom, "Groom", Ambient, (3.0, 5.0), 10.0, 5.0, 0, true),
    d(WashPaws, "Wash paws", Ambient, (3.0, 4.5), 14.0, 4.0, 0, true),
    d(Scratch, "Scratch", Ambient, (2.0, 3.0), 10.0, 4.0, 0, true),
    d(Sniff, "Sniff", Ambient, (2.2, 3.6), 6.0, 6.0, 0, true),
    d(Dig, "Rummage", Ambient, (3.0, 5.0), 18.0, 3.0, 0, true),
    d(Snack, "Snack", Ambient, (4.0, 5.5), 30.0, 2.5, 0, true),
    d(Wave, "Wave", Social, (1.6, 2.0), 15.0, 4.0, 0, true),
    d(Dance, "Dance", Ambient, (4.0, 6.0), 30.0, 1.6, 0, true),
    d(TailChase, "Chase tail", Ambient, (2.4, 3.4), 35.0, 1.5, 0, true),
    d(RollOver, "Roll over", Ambient, (2.5, 4.0), 30.0, 1.8, 0, true),
    d(Sneeze, "Sneeze", Ambient, (1.1, 1.1), 40.0, 1.2, 0, true),
    d(Hop, "Hop", Ambient, (0.0, 0.0), 6.0, 2.5, 0, true),
    d(Flip, "Backflip", Ambient, (0.0, 0.0), 18.0, 1.2, 1, false),
    d(Curious, "Curious", Social, (2.0, 3.5), 6.0, 10.0, 0, true),
    d(Wander, "Wander", Ambient, UNTIL, 0.0, 18.0, 0, true),
    d(Run, "Run", Ambient, UNTIL, 4.0, 5.0, 0, true),
    d(Explore, "Explore", Ambient, UNTIL, 6.0, 9.0, 0, true),
    d(Jump, "Jump", Internal, UNTIL, 0.0, 0.0, 3, false),
    d(Climb, "Climb", Internal, UNTIL, 0.0, 0.0, 3, false),
    d(Mantle, "Mantle", Internal, (0.35, 0.35), 0.0, 0.0, 3, false),
    d(FollowCursor, "Follow cursor", Internal, UNTIL, 0.0, 0.0, 0, true),
    d(ChaseCursor, "Chase cursor", Social, (3.0, 6.0), 12.0, 6.0, 1, true),
    d(Stalk, "Stalk", Social, (1.2, 2.2), 14.0, 4.0, 1, true),
    d(Pounce, "Pounce", Internal, UNTIL, 0.0, 0.0, 3, false),
    d(Fall, "Fall", Internal, UNTIL, 0.0, 0.0, 6, false),
    d(Land, "Land", Internal, (0.32, 0.32), 0.0, 0.0, 4, false),
    d(Alert, "Alert", Reaction, (1.0, 1.0), 8.0, 0.0, 3, true),
    d(Panic, "Panic", Reaction, (3.5, 5.0), 20.0, 0.0, 4, true),
    d(Hot, "Overheating", Reaction, (4.0, 6.0), 45.0, 0.0, 3, true),
    d(Tired, "Low battery", Reaction, (5.0, 7.0), 120.0, 0.0, 3, true),
    d(Cheer, "Cheer", Reaction, (1.8, 1.8), 15.0, 0.0, 4, true),
    d(Stuffed, "Memory full", Reaction, (4.0, 5.0), 150.0, 0.0, 3, true),
    d(Watch, "Downloading", Reaction, (3.0, 4.0), 60.0, 0.0, 2, true),
    d(Petted, "Petted", Reaction, (1.2, 1.2), 0.0, 0.0, 5, true),
    d(Booped, "Booped", Reaction, (0.9, 0.9), 0.0, 0.0, 5, true),
    d(Annoyed, "Annoyed", Reaction, (2.2, 2.2), 0.0, 0.0, 6, false),
    d(Dragged, "Carried", Internal, UNTIL, 0.0, 0.0, 9, false),
    d(Thrown, "Thrown", Internal, UNTIL, 0.0, 0.0, 9, false),
    d(Bounce, "Bounce", Internal, UNTIL, 0.0, 0.0, 8, false),
    d(Splat, "Splat", Internal, (0.75, 0.75), 0.0, 0.0, 8, false),
    d(Dizzy, "Dizzy", Internal, (2.4, 2.4), 0.0, 0.0, 7, true),
    d(GetUp, "Get up", Internal, (0.6, 0.6), 0.0, 0.0, 7, false),
];

pub const COUNT: usize = 50;

pub fn all() -> &'static [Def] {
    DEFS
}

pub fn def(act: Act) -> &'static Def {
    // The table is in enum order, which a test checks.
    &DEFS[act as usize]
}

impl Act {
    pub fn label(self) -> &'static str {
        def(self).label
    }

    /// Activities where he is up in the air and gravity is in charge.
    pub fn airborne(self) -> bool {
        matches!(self, Jump | Pounce | Fall | Thrown | Bounce | Hop | Flip)
    }

    /// Quiet activities a sleep or a reaction may freely replace.
    pub fn calm(self) -> bool {
        matches!(
            self,
            Idle | LookAround | Sit | Loaf | Groom | WashPaws | Sniff | Curious | Wander | Yawn | Tired
        )
    }

    pub fn handled(self) -> bool {
        matches!(self, Dragged | Thrown | Bounce | Splat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_in_enum_order() {
        assert_eq!(DEFS.len(), COUNT);
        for (i, d) in DEFS.iter().enumerate() {
            assert_eq!(d.act as usize, i, "{} is out of order", d.label);
        }
    }

    #[test]
    fn labels_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for d in DEFS {
            assert!(seen.insert(d.label), "duplicate label {}", d.label);
        }
    }

    #[test]
    fn durations_are_sane() {
        for d in DEFS {
            assert!(d.dur.0 <= d.dur.1, "{} has an inverted range", d.label);
            assert!(d.dur.0 >= 0.0);
        }
    }
}
