use std::{error::Error, fmt, str::FromStr};

use crate::world::{Cell, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Laws {
    Conway { birth: u16, survival: u16 },
    Cyclic,
    Transport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    Set(Cell),
    /// The destination must be adjacent and Empty in the current world.
    Move {
        x: usize,
        y: usize,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct RuleError(String);

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid rule: {}; expected B<0-8>/S<0-8> with no repeated digits",
            self.0
        )
    }
}

impl Error for RuleError {}

impl Default for Laws {
    fn default() -> Self {
        Self::Conway {
            birth: 1 << 3,
            survival: (1 << 2) | (1 << 3),
        }
    }
}

impl Laws {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Conway { .. } => "conway",
            Self::Cyclic => "cyclic",
            Self::Transport => "transport",
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Conway { birth, survival } => {
                let digits = |mask: u16| -> String {
                    (0..=8)
                        .filter(|count| mask & (1 << count) != 0)
                        .map(|count| char::from(b'0' + count))
                        .collect()
                };
                format!("B{}/S{}", digits(*birth), digits(*survival))
            }
            Self::Cyclic => "A -> B -> C -> A".to_owned(),
            Self::Transport => "Particle transport".to_owned(),
        }
    }

    pub fn next(&self, world: &World, x: usize, y: usize, generation: u64) -> Transition {
        let current = world.get(x, y);
        let next = match self {
            Self::Conway { birth, survival } => {
                let neighbors = world.neighbors(x, y);
                let count = neighbors.iter().filter(|&&cell| cell == Cell::A).count();
                let mask = if current == Cell::A {
                    *survival
                } else {
                    *birth
                };
                if mask & (1 << count) != 0 {
                    Cell::A
                } else {
                    Cell::Empty
                }
            }
            Self::Cyclic => {
                let neighbors = world.neighbors(x, y);
                let successor = match current {
                    Cell::Empty => return Transition::Set(Cell::Empty),
                    Cell::A => Cell::B,
                    Cell::B => Cell::C,
                    Cell::C => Cell::A,
                };
                if neighbors.contains(&successor) {
                    successor
                } else {
                    current
                }
            }
            Self::Transport => {
                if current != Cell::Empty {
                    let positions = World::neighbor_positions(x, y);
                    let start = (generation % positions.len() as u64) as usize;
                    for offset in 0..positions.len() {
                        let (nx, ny) = positions[(start + offset) % positions.len()];
                        if world.get(nx, ny) == Cell::Empty {
                            return Transition::Move { x: nx, y: ny };
                        }
                    }
                }
                current
            }
        };
        Transition::Set(next)
    }
}

impl FromStr for Laws {
    type Err = RuleError;

    fn from_str(rule: &str) -> Result<Self, Self::Err> {
        let invalid = || RuleError(rule.to_owned());
        let (birth, survival) = rule.split_once('/').ok_or_else(invalid)?;
        let birth = birth.strip_prefix('B').ok_or_else(invalid)?;
        let survival = survival.strip_prefix('S').ok_or_else(invalid)?;
        let parse_mask = |digits: &str| -> Result<u16, RuleError> {
            let mut mask = 0;
            for digit in digits.bytes() {
                if !(b'0'..=b'8').contains(&digit) {
                    return Err(invalid());
                }

                let bit = 1 << (digit - b'0');
                if mask & bit != 0 {
                    return Err(invalid());
                }
                mask |= bit;
            }
            Ok(mask)
        };
        Ok(Self::Conway {
            birth: parse_mask(birth)?,
            survival: parse_mask(survival)?,
        })
    }
}
