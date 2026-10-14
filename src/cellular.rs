use crate::{
    law_system::{LawSystem, SimulationError},
    laws::{Laws, Transition},
    world::{Cell, HEIGHT, WIDTH, World},
};

impl LawSystem for Laws {
    type State = World;
    type Workspace = Vec<Option<usize>>;
    type Baseline = [usize; 4];
    type Observation = [usize; 4];

    fn stages(&self) -> &'static [&'static str] {
        &["cell transitions"]
    }

    fn workspace(&self, _: &World) -> Self::Workspace {
        vec![None; WIDTH * HEIGHT]
    }

    fn validate_workspace(&self, _: &World, workspace: &Self::Workspace) -> Result<(), SimulationError> {
        if workspace.len() != WIDTH * HEIGHT || workspace.iter().flatten().any(|&position| position >= WIDTH * HEIGHT) {
            return Err(SimulationError::new("invalid cellular workspace"));
        }
        Ok(())
    }

    fn baseline(&self, initial: &World) -> Result<Self::Baseline, SimulationError> {
        if initial.cells().len() != WIDTH * HEIGHT || initial.cells().iter().any(|&cell| cell > 3) {
            return Err(SimulationError::new("invalid initial cellular grid"));
        }
        Ok(initial.populations())
    }

    fn advance(
        &self,
        stage: usize,
        current: &World,
        next: &mut World,
        winners: &mut Self::Workspace,
        generation: u64,
    ) -> Result<(), SimulationError> {
        if stage != 0 {
            return Err(SimulationError::new("unknown cellular stage"));
        }
        winners.fill(None);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                match self.next(current, x, y, generation) {
                    Transition::Set(state) => next.set(x, y, state),
                    Transition::Move { x: nx, y: ny } => {
                        next.set(x, y, current.get(x, y));
                        winners[ny * WIDTH + nx].get_or_insert(y * WIDTH + x);
                    }
                }
            }
        }
        for (target, &source) in winners.iter().enumerate() {
            if let Some(source) = source {
                next.set(source % WIDTH, source / WIDTH, Cell::Empty);
                next.set(
                    target % WIDTH,
                    target / WIDTH,
                    current.get(source % WIDTH, source / WIDTH),
                );
            }
        }
        Ok(())
    }

    fn validate(&self, state: &World, baseline: &Self::Baseline) -> Result<(), SimulationError> {
        if state.cells().len() != WIDTH * HEIGHT || state.cells().iter().any(|&cell| cell > 3) {
            return Err(SimulationError::new("invalid cellular grid"));
        }
        if *self == Self::Transport && state.populations() != *baseline {
            return Err(SimulationError::new(
                "transport changed particle populations",
            ));
        }
        Ok(())
    }

    fn observe(&self, state: &World) -> Result<Self::Observation, SimulationError> {
        Ok(state.populations())
    }
}
