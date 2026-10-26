//! 프레임마다 목표값으로 다가가는 간단한 애니메이션 값.

/// 기준 프레임(60Hz, 16.7ms)마다 남은 거리의 이 비율만큼 다가간다. 실제 경과 시간에 맞춰 환산한다.
pub const RATE: f32 = 0.25;
/// `RATE`의 기준 프레임 길이
pub const BASE_FRAME_MS: f32 = 1000.0 / 60.0;
/// 이보다 가까우면 도착한 것으로 본다.
pub const EPSILON: f32 = 0.002;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anim {
    pub value: f32,
    pub target: f32,
}

impl Anim {
    pub const fn at(value: f32) -> Anim {
        Anim { value, target: value }
    }

    pub fn set(&mut self, target: f32) {
        self.target = target;
    }

    /// `dt_ms`만큼 진행 (주사율·프레임 간격이 달라도 같은 속도). 아직 움직이는 중이면 true.
    pub fn step(&mut self, dt_ms: f32) -> bool {
        let diff = self.target - self.value;
        if diff.abs() <= EPSILON {
            self.value = self.target;
            return false;
        }
        let k = 1.0 - (1.0 - RATE).powf((dt_ms / BASE_FRAME_MS).clamp(0.0, 4.0));
        self.value += diff * k;
        true
    }
}

/// 0→1로 일정 시간 동안 진행하는 값 (테마 페이드, 선택기 펼치기). 부드럽게 감속한다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tween {
    pub elapsed_ms: f32,
    pub duration_ms: f32,
}

impl Tween {
    pub const fn new(duration_ms: f32) -> Tween {
        Tween { elapsed_ms: 0.0, duration_ms }
    }

    pub fn done() -> Tween {
        Tween { elapsed_ms: 1.0, duration_ms: 1.0 }
    }

    pub fn advance(&mut self, ms: f32) -> bool {
        self.elapsed_ms = (self.elapsed_ms + ms).min(self.duration_ms);
        self.elapsed_ms < self.duration_ms
    }

    /// 감속 곡선(ease-out cubic)을 적용한 0..1
    pub fn progress(&self) -> f32 {
        let t = (self.elapsed_ms / self.duration_ms).clamp(0.0, 1.0);
        1.0 - (1.0 - t).powi(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anim_converges_and_stops() {
        let mut a = Anim::at(1.0);
        a.set(1.15);
        let mut frames = 0;
        while a.step(BASE_FRAME_MS) {
            frames += 1;
            assert!(frames < 100);
        }
        assert_eq!(a.value, 1.15);
        assert!(frames > 5);
        assert!(!a.step(BASE_FRAME_MS));
    }

    #[test]
    fn anim_speed_does_not_depend_on_frame_rate() {
        // 60Hz로 10프레임 = 144Hz로 24프레임 (같은 시간)
        let (mut slow, mut fast) = (Anim::at(0.0), Anim::at(0.0));
        slow.set(1.0);
        fast.set(1.0);
        for _ in 0..10 {
            slow.step(BASE_FRAME_MS);
        }
        for _ in 0..24 {
            fast.step(1000.0 / 144.0);
        }
        assert!((slow.value - fast.value).abs() < 0.01, "{} vs {}", slow.value, fast.value);
    }

    #[test]
    fn tween_eases_out_and_finishes() {
        let mut t = Tween::new(160.0);
        assert_eq!(t.progress(), 0.0);
        assert!(t.advance(80.0));
        assert!(t.progress() > 0.5); // 감속: 절반 시간에 절반보다 많이 진행
        assert!(!t.advance(100.0));
        assert_eq!(t.progress(), 1.0);
        assert_eq!(Tween::done().progress(), 1.0);
    }
}
