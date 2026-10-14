use serde::{Deserialize, Serialize};
use crate::law_system::SimulationError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Torus {
    pub width: usize,
    pub height: usize,
}

impl Default for Torus {
    fn default() -> Self { Self { width: 64, height: 64 } }
}

impl Torus {
    pub fn validate(self) -> Result<(), SimulationError> {
        if self.width < 8 || self.height < 8 || self.width > 512 || self.height > 512 {
            return Err(SimulationError::new("world dimensions must be between 8 and 512"));
        }
        Ok(())
    }
    pub fn len(self) -> usize { self.width * self.height }
    pub fn neighbor(self, position: usize, direction: usize) -> usize {
        let x = position % self.width;
        let y = position / self.width;
        match direction % 4 {
            0 => y * self.width + (x + 1) % self.width,
            1 => ((y + 1) % self.height) * self.width + x,
            2 => y * self.width + (x + self.width - 1) % self.width,
            _ => ((y + self.height - 1) % self.height) * self.width + x,
        }
    }
    pub fn offset(self, mut position: usize, direction: usize, distance: usize) -> usize {
        for _ in 0..distance { position = self.neighbor(position, direction); }
        position
    }
    pub fn adjacent(self, a: usize, b: usize) -> bool {
        (0..4).any(|direction| self.neighbor(a, direction) == b)
    }
}

pub fn add(value: &mut u64, amount: u64) -> Result<(), SimulationError> {
    *value = value.checked_add(amount).ok_or_else(|| SimulationError::new("counter overflow"))?;
    Ok(())
}
