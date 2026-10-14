use crate::{chemistry::{ChemistryConfig, ChemistryState, Probability, ReactionPattern, ReactionRule},
    law_system::SimulationError, universe::Universe, world::{Cell, HEIGHT, WIDTH, World}};

/// Small recorded initial conditions; never run automatically.
pub fn chemistry(name: &str, seed: u64) -> Result<Universe, SimulationError> {
    let mut config = ChemistryConfig { seed, ..ChemistryConfig::default() };
    let mut grid = World::new();
    match name {
        "exchange" => {
            grid.set(6, 6, Cell::A); grid.set(7, 6, Cell::B); grid.set(7, 7, Cell::C);
            config.movement_energy = u64::MAX;
            config.rules = vec![ReactionRule { pattern: ReactionPattern::Exchange { leaving: Cell::A, center: Cell::B, incoming: Cell::C },
                probability: Probability { numerator: 1, denominator: 1 }, priority: 0 }];
        }
        "toroidal-group" => {
            grid.set(WIDTH - 1, 6, Cell::B); grid.set(0, 6, Cell::C);
            config.rules.clear();
        }
        "overlapping-footprint" => {
            grid.set(6, 6, Cell::B); grid.set(7, 6, Cell::C);
            config.rules.clear();
        }
        "competing-groups" => {
            for (x, y, kind) in [(6,6,Cell::B),(7,6,Cell::C),(9,6,Cell::B),(10,6,Cell::C)] { grid.set(x,y,kind); }
            config.rules.clear();
        }
        _ => return Err(SimulationError::new("unknown chemistry scenario")),
    }
    let mut state = ChemistryState::new(grid, vec![8; WIDTH * HEIGHT])?;
    state.add_initial_bond(1, 2)?;
    if name == "competing-groups" { state.add_initial_bond(3, 4)?; }
    Universe::from_chemistry_state(config, state)
}
