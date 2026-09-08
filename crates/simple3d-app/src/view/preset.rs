//! The named viewpoints, and moving the camera to one.

/// The standard view presets (spec section 6.1). Yaw and pitch in degrees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewPreset {
    Top,
    Bottom,
    Front,
    Back,
    Left,
    Right,
    Isometric,
}

impl ViewPreset {
    /// Yaw and pitch that put the camera on the named side. The camera looks
    /// along `-offset_dir`, so "front" (looking at the XZ plane from -Y) means
    /// the eye sits at -Y, i.e. yaw = -90 degrees.
    pub fn angles(self) -> (f64, f64) {
        match self {
            ViewPreset::Top => (-90.0, 89.9),
            ViewPreset::Bottom => (-90.0, -89.9),
            ViewPreset::Front => (-90.0, 0.0),
            ViewPreset::Back => (90.0, 0.0),
            ViewPreset::Right => (0.0, 0.0),
            ViewPreset::Left => (180.0, 0.0),
            ViewPreset::Isometric => (-55.0, 28.0),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ViewPreset::Top => "Top",
            ViewPreset::Bottom => "Bottom",
            ViewPreset::Front => "Front",
            ViewPreset::Back => "Back",
            ViewPreset::Left => "Left",
            ViewPreset::Right => "Right",
            ViewPreset::Isometric => "Isometric",
        }
    }
}

/// The shortest way round from one yaw to another, in degrees. Turning from
/// 170 to -170 is twenty degrees, not three hundred and forty: a view cube that
/// spins the long way round to an adjacent face reads as a glitch.
pub fn shortest_turn(from: f64, to: f64) -> f64 {
    let mut delta = (to - from) % 360.0;
    if delta > 180.0 {
        delta -= 360.0;
    }
    if delta < -180.0 {
        delta += 360.0;
    }
    delta
}

/// The transition curve: ease in and out, so the camera starts and stops rather
/// than jumping into motion at full speed.
pub fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How long a view change takes. The design's number, and short enough that it
/// reads as the same camera moving rather than as a wait.
pub const TRANSITION: std::time::Duration = std::time::Duration::from_millis(200);

/// A camera turn in flight.
#[derive(Clone, Copy, Debug)]
pub struct CameraMove {
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub started: std::time::Instant,
}

impl CameraMove {
    /// Yaw and pitch at this moment, and whether the move is over.
    pub fn at(&self, now: std::time::Instant) -> ((f64, f64), bool) {
        let t = now.duration_since(self.started).as_secs_f64() / TRANSITION.as_secs_f64();
        let done = t >= 1.0;
        let e = ease(t);
        ((self.from.0 + (self.to.0 - self.from.0) * e, self.from.1 + (self.to.1 - self.from.1) * e), done)
    }
}
