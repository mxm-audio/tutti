//! The time arithmetic the spec defines: ticks to seconds and back through the tempo map
//! (spec 5.2), and an audio piece's placement (spec 5.5). Everything here is a pure function of
//! the document's numbers; the rounding rules are the spec's.

use crate::model::{Curve, MapPoint, TempoPoint};

/// The ticks a document may hold: 0 to 2^40 (spec 5.2).
pub const TICK_MAX: i64 = 1 << 40;

/// `round_half_up`: the sample a time falls on (spec 5.2).
pub fn round_half_up(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

/// A tempo map ready to integrate: each point's seconds from tick 0, summed in tick order.
#[derive(Clone, Debug)]
pub struct TempoMap {
    ppq: f64,
    points: Vec<(i64, f64, Curve)>,
    /// Seconds at each point, from tick 0; the first is 0.
    starts: Vec<f64>,
}

impl TempoMap {
    /// From a document's `ppq` and `tempo`. The points are taken as given: `validate` is what
    /// refuses an unordered or empty map; an empty map here is 120 bpm.
    pub fn new(ppq: u32, tempo: &[TempoPoint]) -> Self {
        let mut points: Vec<(i64, f64, Curve)> = tempo
            .iter()
            .map(|p| (p.at, p.bpm, p.curve.effective()))
            .collect();
        if points.is_empty() {
            points.push((0, 120.0, Curve::Hold));
        }
        let ppq = ppq as f64;
        let mut starts = Vec::with_capacity(points.len());
        let mut total = 0.0;
        for i in 0..points.len() {
            starts.push(total);
            if i + 1 < points.len() {
                let (a, bpm_a, curve) = &points[i];
                let (b, bpm_b, _) = &points[i + 1];
                total += segment_seconds(ppq, *a, *bpm_a, curve, *b, *bpm_b, *b as f64);
            }
        }
        Self {
            ppq,
            points,
            starts,
        }
    }

    /// The index of the segment a tick falls in: the last point at or before it (the first for
    /// a tick before tick 0).
    fn segment(&self, tick: f64) -> usize {
        self.points
            .iter()
            .rposition(|(at, _, _)| (*at as f64) <= tick)
            .unwrap_or(0)
    }

    /// The tempo in force at a tick.
    pub fn bpm_at(&self, tick: f64) -> f64 {
        let i = self.segment(tick);
        let (a, bpm_a, curve) = &self.points[i];
        match (curve, self.points.get(i + 1)) {
            (Curve::Linear, Some((b, bpm_b, _))) if tick >= *a as f64 => {
                let k = (bpm_b - bpm_a) / ((*b - *a) as f64);
                bpm_a + k * (tick - *a as f64)
            }
            _ => *bpm_a,
        }
    }

    /// Seconds from tick 0 to a tick (negative before tick 0, where the first tempo holds).
    pub fn seconds_at(&self, tick: f64) -> f64 {
        let i = self.segment(tick);
        let (a, bpm_a, curve) = &self.points[i];
        let partial = match self.points.get(i + 1) {
            Some((b, bpm_b, _)) if tick >= *a as f64 => {
                segment_seconds(self.ppq, *a, *bpm_a, curve, *b, *bpm_b, tick)
            }
            _ => 60.0 * (tick - *a as f64) / (self.ppq * bpm_a),
        };
        self.starts[i] + partial
    }

    /// The tick at a number of seconds from tick 0: the inverse of [`Self::seconds_at`].
    pub fn tick_at(&self, seconds: f64) -> f64 {
        let i = self.starts.iter().rposition(|s| *s <= seconds).unwrap_or(0);
        let (a, bpm_a, curve) = &self.points[i];
        let s = seconds - self.starts[i];
        match (curve, self.points.get(i + 1)) {
            (Curve::Linear, Some((b, bpm_b, _))) if seconds >= self.starts[i] && bpm_b != bpm_a => {
                let k = (bpm_b - bpm_a) / ((*b - *a) as f64);
                *a as f64 + bpm_a * ((s * self.ppq * k / 60.0).exp() - 1.0) / k
            }
            _ => *a as f64 + s * self.ppq * bpm_a / 60.0,
        }
    }

    /// The sample a tick falls on at a rate (spec 5.2: `round_half_up(seconds · rate)`).
    pub fn samples_at(&self, tick: f64, rate: u32) -> i64 {
        round_half_up(self.seconds_at(tick) * rate as f64)
    }
}

/// Seconds from tick `a` (at `bpm_a`, running `curve` to tick `b` at `bpm_b`) to tick `t` in
/// `[a, b]`: the spec's closed forms.
fn segment_seconds(ppq: f64, a: i64, bpm_a: f64, curve: &Curve, b: i64, bpm_b: f64, t: f64) -> f64 {
    let dt = t - a as f64;
    match curve {
        Curve::Linear if bpm_b != bpm_a && b != a => {
            let k = (bpm_b - bpm_a) / ((b - a) as f64);
            60.0 / (ppq * k) * ((bpm_a + k * dt) / bpm_a).ln()
        }
        _ => 60.0 * dt / (ppq * bpm_a),
    }
}

/// An audio piece's map and bpm, ready to place frames (spec 5.5).
#[derive(Clone, Debug)]
pub struct Placement {
    ppq: f64,
    rate: f64,
    points: Vec<(f64, f64)>,
    bpm: Option<f64>,
}

impl Placement {
    pub fn new(ppq: u32, sample_rate: u32, map: &[MapPoint], bpm: Option<f64>) -> Self {
        Self {
            ppq: ppq as f64,
            rate: sample_rate as f64,
            points: map
                .iter()
                .map(|p| (p.frame as f64, p.tick as f64))
                .collect(),
            bpm,
        }
    }

    /// `q`, the slope outside the points, in ticks per source frame: `ppq · bpm / (60 · rate)`.
    /// `None` for an unwarped piece (`bpm: null`), whose content is placed in seconds.
    pub fn slope(&self) -> Option<f64> {
        self.bpm.map(|bpm| self.ppq * bpm / (60.0 * self.rate))
    }

    /// The pattern tick a source frame sits at, or `None` for an unwarped piece.
    pub fn tick_of_frame(&self, frame: f64) -> Option<f64> {
        let q = self.slope()?;
        let (first, last) = (self.points.first()?, self.points.last()?);
        if frame <= first.0 {
            return Some(first.1 + (frame - first.0) * q);
        }
        if frame >= last.0 {
            return Some(last.1 + (frame - last.0) * q);
        }
        let i = self.points.iter().rposition(|(f, _)| *f <= frame)?;
        let (f0, t0) = self.points[i];
        let (f1, t1) = self.points[i + 1];
        Some(t0 + (frame - f0) * (t1 - t0) / (f1 - f0))
    }

    /// The source frame at a pattern tick, or `None` for an unwarped piece.
    pub fn frame_at_tick(&self, tick: f64) -> Option<f64> {
        let q = self.slope()?;
        let (first, last) = (self.points.first()?, self.points.last()?);
        if tick <= first.1 {
            return Some(first.0 + (tick - first.1) / q);
        }
        if tick >= last.1 {
            return Some(last.0 + (tick - last.1) / q);
        }
        let i = self.points.iter().rposition(|(_, t)| *t <= tick)?;
        let (f0, t0) = self.points[i];
        let (f1, t1) = self.points[i + 1];
        Some(f0 + (tick - t0) * (f1 - f0) / (t1 - t0))
    }

    /// The source frames per output second at a tick, given the project's tempo there:
    /// `ppq · B / (60 · q)` with `q` the map's slope at that tick; an unwarped piece runs at its
    /// own rate.
    pub fn rate_at(&self, tick: f64, project_bpm: f64) -> f64 {
        match self.slope_at(tick) {
            Some(q) => self.ppq * project_bpm / (60.0 * q),
            None => self.rate,
        }
    }

    /// The map's slope at a tick: between two points theirs, outside them `q`.
    fn slope_at(&self, tick: f64) -> Option<f64> {
        let q = self.slope()?;
        let (first, last) = (self.points.first()?, self.points.last()?);
        if tick < first.1 || tick >= last.1 || self.points.len() < 2 {
            return Some(q);
        }
        let i = self.points.iter().rposition(|(_, t)| *t <= tick)?;
        let (f0, t0) = self.points[i];
        let (f1, t1) = self.points[i + 1];
        Some((t1 - t0) / (f1 - f0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Unknown;

    fn point(at: i64, bpm: f64, curve: Curve) -> TempoPoint {
        TempoPoint {
            at,
            bpm,
            curve,
            unknown: Unknown::new(),
        }
    }

    #[test]
    fn the_spec_vectors_for_a_linear_ramp() {
        // 120 to 60 bpm over four beats at ppq 960, then 60 held (spec 5.2).
        let map = TempoMap::new(
            960,
            &[
                point(0, 120.0, Curve::Linear),
                point(3840, 60.0, Curve::Hold),
            ],
        );
        let want = [25638, 55235, 90241, 133084, 181084];
        for (beat, sample) in (1..=5).zip(want) {
            assert_eq!(
                map.samples_at(beat as f64 * 960.0, 48_000),
                sample,
                "beat {beat}"
            );
        }
        let seconds = [0.534126, 1.150728, 1.880015, 2.772589, 3.772589];
        for (beat, s) in (1..=5).zip(seconds) {
            assert!(
                (map.seconds_at(beat as f64 * 960.0) - s).abs() < 1e-6,
                "beat {beat}"
            );
        }
        // The inverse comes back to the tick.
        for tick in [0.0, 500.0, 960.0, 3839.0, 3840.0, 4800.0, 9999.0] {
            assert!(
                (map.tick_at(map.seconds_at(tick)) - tick).abs() < 1e-6,
                "{tick}"
            );
        }
        assert_eq!(map.bpm_at(1920.0), 90.0);
        assert_eq!(map.bpm_at(9999.0), 60.0);
    }

    #[test]
    fn hold_segments_are_exact_and_the_first_tempo_holds_before_zero() {
        let map = TempoMap::new(
            960,
            &[point(0, 120.0, Curve::Hold), point(1920, 60.0, Curve::Hold)],
        );
        assert_eq!(map.seconds_at(1920.0), 1.0);
        assert_eq!(map.seconds_at(3840.0), 3.0);
        assert_eq!(map.seconds_at(-960.0), -0.5);
        assert_eq!(map.samples_at(1920.0, 44_100), 44_100);
        assert_eq!(round_half_up(2.5), 3);
        assert_eq!(round_half_up(2.4999), 2);
    }

    #[test]
    fn a_loop_at_95_in_a_project_at_87() {
        // The spec's worked example (5.5): 222 821 frames at 44.1 kHz, one point, bpm 95.
        let map = [MapPoint {
            frame: 0,
            tick: 0,
            unknown: Unknown::new(),
        }];
        let placement = Placement::new(960, 44_100, &map, Some(95.0));
        let q = placement.slope().unwrap();
        assert!((q - 0.034467).abs() < 1e-6);
        let last = placement.tick_of_frame(222_821.0).unwrap();
        assert_eq!(last.round() as i64, 7680);
        assert!((placement.frame_at_tick(7680.0).unwrap() - 222_821.0).abs() < 1.0);
        // At 87 bpm the piece advances 87/95 of a source frame per output frame.
        let per_second = placement.rate_at(100.0, 87.0);
        assert!((per_second / 44_100.0 - 87.0 / 95.0).abs() < 1e-9);
        // Unwarped: no tick slope, content at its own rate.
        let unwarped = Placement::new(960, 44_100, &map, None);
        assert!(unwarped.tick_of_frame(10.0).is_none());
        assert_eq!(unwarped.rate_at(0.0, 87.0), 44_100.0);
    }

    #[test]
    fn a_warp_follows_its_points_between_them_and_the_bpm_beyond() {
        let map = [
            MapPoint {
                frame: 0,
                tick: 0,
                unknown: Unknown::new(),
            },
            MapPoint {
                frame: 44_100,
                tick: 1920,
                unknown: Unknown::new(),
            },
        ];
        let p = Placement::new(960, 44_100, &map, Some(120.0));
        assert_eq!(p.tick_of_frame(22_050.0).unwrap(), 960.0);
        assert_eq!(p.frame_at_tick(960.0).unwrap(), 22_050.0);
        // Beyond the last point: q = 960 * 120 / (60 * 44100) ticks per frame.
        let q = 960.0 * 120.0 / (60.0 * 44_100.0);
        assert!((p.tick_of_frame(88_200.0).unwrap() - (1920.0 + 44_100.0 * q)).abs() < 1e-9);
    }
}
