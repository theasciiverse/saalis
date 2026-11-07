//! Cues: the moments the game wants heard and felt (Kaiku's pattern). The
//! simulation only names them; the main loop turns each into a sound and a
//! rumble, so the simulation stays silent and testable.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Cue {
    /// the player's guns
    Gun,
    /// someone else's guns, within earshot
    EnemyGun,
    /// the player's shields took a hit; the armour under them did
    ShieldHit,
    ArmourHit,
    /// a shot of ours landed
    HitConfirm,
    Kill,
    BigKill,
    PlayerLost,
    /// afterburner lit
    Burner,
    /// a new target selected
    Target,
    /// a message on the comms line
    Comms,
    /// a new wave closing in
    Contact,
    /// out of energy: the guns click
    Empty,
    /// a missile away; one locked on to the player
    Missile,
    Warning,
}
