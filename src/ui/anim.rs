//! 프레임마다 목표값으로 다가가는 간단한 애니메이션 값.

/// 한 프레임(16ms)에 남은 거리의 이 비율만큼 다가간다.
pub const RATE: f32 = 0.25;
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

    /// 한 프레임 진행. 아직 움직이는 중이면 true.
    pub fn step(&mut self) -> bool {
        let diff = self.target - self.value;
        if diff.abs() <= EPSILON {
            self.value = self.target;
            return false;
        }
        self.value += diff * RATE;
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
        while a.step() {
            frames += 1;
            assert!(frames < 100);
        }
        assert_eq!(a.value, 1.15);
        assert!(frames > 5);
        assert!(!a.step());
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
