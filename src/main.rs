mod state;
use state::State;

fn main() {
    let mut state = State::restore();
    state.current_term += 1;
    state.persist();
}
